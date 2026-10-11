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
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// A job: run once by every worker, given its index.
pub(crate) type Job<'env> = Arc<dyn Fn(usize) + Send + Sync + 'env>;

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
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a worker panicked".to_owned())
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
        let r = f(pool_ref);
        pool.shutdown();
        r
    })
}
