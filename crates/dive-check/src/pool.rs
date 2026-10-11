//! A pool of worker threads that lives for one search (ADR 0015 §2).
//!
//! Each phase of each level is a *job*: a closure every thread runs once,
//! with its worker index, and which claims its own work (blocks, chunks)
//! through atomics. The coordinator, the thread that owns the search, runs
//! the job as worker 0 and returns once every worker has finished it: that
//! is the barrier between phases.
//!
//! Workers are spawned once, so a search with thousands of levels does not
//! spawn thousands of threads. Jobs own their level's data through `Arc`s;
//! what they borrow (the model, the seen-state set behind its lock) lives
//! as long as the pool. No `unsafe` code is involved.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// A job: run once by every worker, given its index.
pub(crate) type Job<'env> = Arc<dyn Fn(usize) + Send + Sync + 'env>;

struct PauseReset<'a>(&'a AtomicBool);
impl Drop for PauseReset<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct State<'env> {
    /// Incremented for every job.
    generation: u64,
    job: Option<Job<'env>>,
    /// Workers still running the current job (the coordinator excluded).
    running: usize,
    shutdown: bool,
    /// The first panic of a worker, as a message.
    panicked: Option<String>,
    /// Worker threads started (the coordinator excluded).
    workers: usize,
}

/// The pool. See the module documentation.
pub(crate) struct Pool<'env> {
    state: Mutex<State<'env>>,
    start: Condvar,
    done: Condvar,
    /// Threads requested, the coordinator included.
    threads: usize,
    paused: AtomicBool,
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a worker panicked".to_owned())
}

struct Shutdown<'a, 'env>(&'a Pool<'env>);
impl Drop for Shutdown<'_, '_> {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

impl<'env> Pool<'env> {
    fn new(threads: usize) -> Self {
        Self {
            state: Mutex::new(State {
                generation: 0,
                job: None,
                running: 0,
                shutdown: false,
                panicked: None,
                workers: 0,
            }),
            start: Condvar::new(),
            done: Condvar::new(),
            threads,
            paused: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State<'env>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Threads running jobs, the coordinator included.
    pub(crate) fn threads(&self) -> usize {
        self.lock().workers + 1
    }

    /// Runs `job` on every thread and waits for all of them.
    ///
    /// # Errors
    /// The message of a panic in any thread (a bug).
    pub(crate) fn run(&self, job: &Job<'env>) -> Result<(), String> {
        if self.paused.load(Ordering::Acquire) {
            return Err("job dispatch attempted inside a dispatch pause".into());
        }
        let mut st = self.lock();
        if st.workers > 0 {
            st.generation += 1;
            st.job = Some(job.clone());
            st.running = st.workers;
            drop(st);
            self.start.notify_all();
        } else {
            drop(st);
        }
        let own = catch_unwind(AssertUnwindSafe(|| job(0))).err();
        let mut st = self.lock();
        while st.running > 0 {
            st = self.done.wait(st).unwrap_or_else(PoisonError::into_inner);
        }
        st.job = None;
        let worker = st.panicked.take();
        drop(st);
        match (own, worker) {
            (Some(p), _) => Err(panic_message(&*p)),
            (None, Some(m)) => Err(m),
            (None, None) => Ok(()),
        }
    }

    /// Hold real dispatch authority across a coordinator callback. This does
    /// not suspend a Java state queue or finish the logical frontier.
    pub(crate) fn with_pause<R>(
        &self,
        callback: impl FnOnce(&crate::capture::DispatchPause<'_>) -> R,
    ) -> Result<R, String> {
        if self.paused.swap(true, Ordering::AcqRel) {
            return Err("nested dispatch pause".into());
        }
        let _reset = PauseReset(&self.paused);
        let state = self.lock();
        if state.running != 0 || state.job.is_some() || state.shutdown {
            return Err("dispatch pause requires a joined live pool".into());
        }
        let pause = crate::capture::DispatchPause::new(state.workers + 1, state.generation);
        let result = callback(&pause);
        drop(state);
        Ok(result)
    }

    /// Runs `job` as worker 0 on the calling thread only.
    ///
    /// # Errors
    /// The message of a panic (a bug).
    pub(crate) fn run_inline(job: &Job<'env>) -> Result<(), String> {
        catch_unwind(AssertUnwindSafe(|| job(0))).map_err(|p| panic_message(&*p))
    }

    fn worker(&self, index: usize) {
        let mut seen = 0;
        loop {
            let mut st = self.lock();
            while st.generation == seen && !st.shutdown {
                st = self.start.wait(st).unwrap_or_else(PoisonError::into_inner);
            }
            if st.shutdown {
                return;
            }
            seen = st.generation;
            let Some(job) = st.job.clone() else {
                continue;
            };
            drop(st);
            let r = catch_unwind(AssertUnwindSafe(|| job(index)));
            drop(job);
            let mut st = self.lock();
            if let Err(p) = r {
                st.panicked.get_or_insert_with(|| panic_message(&*p));
            }
            st.running -= 1;
            if st.running == 0 {
                self.done.notify_all();
            }
        }
    }

    fn shutdown(&self) {
        self.lock().shutdown = true;
        self.start.notify_all();
    }
}

/// Runs `f` with a pool of up to `threads` threads (the calling thread
/// included), each worker with `stack` bytes of stack. Fewer threads are
/// used if the system refuses to start more; results never depend on the
/// count.
pub(crate) fn with_pool<'env, R>(
    threads: usize,
    stack: usize,
    f: impl FnOnce(&Pool<'env>) -> R,
) -> R {
    let pool = Pool::new(threads.max(1));
    std::thread::scope(|s| {
        let pool_ref = &pool;
        let mut started = 1;
        for i in 1..pool.threads {
            let spawned = std::thread::Builder::new()
                .name(format!("dive-worker-{i}"))
                .stack_size(stack)
                .spawn_scoped(s, move || pool_ref.worker(i));
            if spawned.is_err() {
                break;
            }
            started += 1;
        }
        pool.lock().workers = started - 1;
        // Coordinator/observer unwind must wake real waiting workers before
        // thread::scope attempts to join them; timer closure alone is insufficient.
        let shutdown = Shutdown(pool_ref);
        let r = f(pool_ref);
        drop(shutdown);
        r
    })
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "controlled pool lifecycle/unwind assertions"
)]
mod pause_tests {
    use super::*;
    #[test]
    fn uncaught_coordinator_pause_unwind_shuts_down_and_joins_waiting_workers() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_pool(4, dive_eval::RECOMMENDED_STACK, |pool| {
                let job: Job<'_> = Arc::new(|_| {});
                pool.run(&job).expect("actual joined job");
                pool.with_pause(|_| panic!("uncaught coordinator pause"))
                    .expect("actual pause");
            });
        }));
        assert!(result.is_err());
    }
    #[test]
    fn pause_owns_mutex_reentry_refuses_and_future_jobs_resume() {
        for width in [1, 3, 4, 8] {
            with_pool(width, dive_eval::RECOMMENDED_STACK, |pool| {
                let visits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let v = visits.clone();
                let job: Job<'_> = Arc::new(move |_| {
                    v.fetch_add(1, Ordering::Relaxed);
                });
                pool.run(&job).expect("joined first job");
                pool.with_pause(|pause| {
                    assert_eq!(pause.workers(), width);
                    assert!(pool.state.try_lock().is_err());
                    assert!(pool.run(&job).is_err());
                    assert!(pool.with_pause(|_| ()).is_err());
                    assert_eq!(visits.load(Ordering::Relaxed), width);
                })
                .expect("actual pause");
                pool.run(&job).expect("resumed second job");
                assert_eq!(visits.load(Ordering::Relaxed), width * 2);
            });
        }
    }
    #[test]
    fn failed_and_panicking_callbacks_release_dispatch_authority() {
        with_pool(4, dive_eval::RECOMMENDED_STACK, |pool| {
            let job: Job<'_> = Arc::new(|_| {});
            assert_eq!(
                pool.with_pause(|_| Err::<(), _>("callback")),
                Ok(Err("callback"))
            );
            let panic = catch_unwind(AssertUnwindSafe(|| {
                pool.with_pause(|_| panic!("controlled failure"))
            }));
            assert!(panic.is_err());
            pool.run(&job).expect("authority released after panic");
        });
    }
}
