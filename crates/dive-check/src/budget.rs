//! Run budgets (ADR 0107): a wall-clock limit and a whole-process
//! resident-memory ceiling, the runaway guards that replace fixed counts.
//!
//! A [`Budget`] says how long a run may take and how much resident memory the
//! process may hold. [`Budget::start`] arms it: a watchdog thread measures
//! elapsed time and the resident set size, and trips a shared flag once
//! either is exceeded. Engines read the flag at points they already pass
//! (each expanded state, each claimed block, each analysis unit) with one
//! relaxed atomic load, so an unexhausted budget costs nothing measurable. A
//! tripped budget stops the run with [`crate::Verdict::Stopped`]: never a
//! verdict about the model, always naming the resource, its limit, what was
//! observed and the phase the run was in.
//!
//! TLC has no such counts. It is bounded by its JVM heap (out of memory is
//! `SYSTEM_OUT_OF_MEMORY`, exit 1 or 255) and has no wall-clock limit except
//! the developer property `-Dtlc2.TLC.stopAfter`. Under `--compat=tlc` the
//! time limit is therefore unset; the memory ceiling stands for TLC's heap.
//!
//! Memory (resident plus swapped pages from `/proc/self/status` on Linux;
//! resident pages from `/bin/ps` on macOS) is read on the watchdog thread
//! only, first after [`MEMORY_EVERY`], so a short run never samples at all.
//! Physical memory is read once per process. Where memory cannot be read, no
//! ceiling is claimed: [`memory_readable`] is false, or a failed sample marks
//! the armed budget ([`Budget::memory_unavailable`]).
//!
//! ```
//! use std::time::Duration;
//! use dive_check::budget::Budget;
//! let budget = Budget::new(Some(Duration::from_secs(3600)), None);
//! let watchdog = budget.start().expect("a limited budget starts a watchdog");
//! assert!(watchdog.budget().poll().is_none());
//! assert!(Budget::unlimited().start().is_none());
//! ```

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The dive profile's default wall-clock limit. Provisional: the release
/// value is chosen from the benchmark campaign before release (ADR 0107).
/// The harness and the gate pass explicit limits; library tests that keep
/// the default finish far below it, and no test depends on its value.
pub const DEFAULT_TIME_LIMIT: Duration = Duration::from_mins(30);

/// How often the watchdog wakes to compare elapsed time with the limit.
const TICK: Duration = Duration::from_millis(50);

/// How often the watchdog samples memory, and how long it waits before the
/// first sample. Reading `/proc` costs microseconds; starting `/bin/ps` on
/// macOS costs milliseconds.
pub const MEMORY_EVERY: Duration = if cfg!(target_os = "linux") {
    Duration::from_millis(100)
} else {
    Duration::from_millis(500)
};

/// The resource a budget limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resource {
    /// Wall-clock time, in milliseconds.
    Time,
    /// Resident memory of the whole process, in bytes.
    Memory,
}

impl Resource {
    /// The name in dive's output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Memory => "memory",
        }
    }

    const fn flag(self) -> u8 {
        match self {
            Self::Time => 1,
            Self::Memory => 2,
        }
    }
}

/// The phase a stopped run was in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Safety search.
    Search,
    /// Behavior-graph capture for temporal checking.
    Capture,
    /// Temporal analysis after capture: planning, labelling, product, search.
    Temporal,
}

impl Phase {
    /// The name in dive's output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Search => "safety-search",
            Self::Capture => "graph-capture",
            Self::Temporal => "temporal-analysis",
        }
    }
}

/// A run stopped by its budget. The statistics at the stop are the result's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stopped {
    /// What ran out.
    pub resource: Resource,
    /// The limit: milliseconds for time, bytes for memory.
    pub limit: u64,
    /// What the watchdog observed when it tripped, in the same unit.
    pub observed: u64,
    /// Milliseconds from arming the budget to the stop the engine reported,
    /// at or after the trip.
    pub elapsed_ms: u64,
    /// The phase the run was in.
    pub phase: Phase,
}

impl Stopped {
    /// How to continue, for people and agents.
    #[must_use]
    pub fn hint(&self) -> &'static str {
        match self.resource {
            Resource::Time => {
                "raise --time-limit, or pass --time-limit=none; the run made no claim about the model"
            }
            Resource::Memory => {
                "raise --max-memory, or pass --max-memory=none; the run made no claim about the model"
            }
        }
    }
}

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.resource {
            Resource::Time => write!(
                f,
                "stopped during {} after {} ms: the time limit is {} ms",
                self.phase.name(),
                self.observed,
                self.limit
            ),
            Resource::Memory => write!(
                f,
                "stopped during {}: resident memory {} bytes exceeds the ceiling of {} bytes",
                self.phase.name(),
                self.observed,
                self.limit
            ),
        }
    }
}

/// What ran out, before the phase is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exhausted {
    /// What ran out.
    pub resource: Resource,
    /// The limit.
    pub limit: u64,
    /// What was observed.
    pub observed: u64,
    /// Milliseconds from arming to this poll.
    pub elapsed_ms: u64,
}

impl Exhausted {
    /// The stop in `phase`.
    #[must_use]
    pub const fn during(self, phase: Phase) -> Stopped {
        Stopped {
            resource: self.resource,
            limit: self.limit,
            observed: self.observed,
            elapsed_ms: self.elapsed_ms,
            phase,
        }
    }
}

#[derive(Debug)]
struct Inner {
    tripped: AtomicU8,
    observed: AtomicU64,
    /// A memory sample failed: the ceiling is not enforced.
    unreadable: AtomicBool,
    started: Instant,
    done: Mutex<bool>,
    wake: Condvar,
}

impl Inner {
    fn trip(&self, resource: Resource, observed: u64) {
        if self.tripped.load(Ordering::Relaxed) == 0 {
            self.observed.store(observed, Ordering::Relaxed);
            self.tripped.store(resource.flag(), Ordering::Release);
        }
    }
}

/// A run's time and memory limits, and once armed, the flag that says
/// whether either is exhausted (see the module documentation).
#[derive(Clone, Debug, Default)]
pub struct Budget {
    /// The wall-clock limit; `None` is unlimited. TLC setting: `None`.
    pub time: Option<Duration>,
    /// The whole-process resident-memory ceiling, in bytes; `None` is
    /// unlimited. TLC setting: its JVM heap ([`default_memory`]).
    pub memory: Option<u64>,
    signal: Option<Arc<Inner>>,
}

impl Budget {
    /// No limit at all.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            time: None,
            memory: None,
            signal: None,
        }
    }

    /// `time` and `memory`, not yet armed.
    #[must_use]
    pub const fn new(time: Option<Duration>, memory: Option<u64>) -> Self {
        Self {
            time,
            memory,
            signal: None,
        }
    }

    /// Whether neither limit is set.
    #[must_use]
    pub const fn is_unlimited(&self) -> bool {
        self.time.is_none() && self.memory.is_none()
    }

    /// Whether this budget is armed, by [`Self::start`] or by a caller that
    /// already started its watchdog.
    #[must_use]
    pub const fn is_armed(&self) -> bool {
        self.signal.is_some()
    }

    /// Arms the budget: starts the watchdog thread and returns it. The
    /// watchdog stops when dropped. `None` for an unlimited or already armed
    /// budget, or if the thread cannot start (the run then goes unguarded,
    /// as without limits).
    #[must_use]
    pub fn start(&self) -> Option<Watchdog> {
        if self.is_unlimited() || self.is_armed() {
            return None;
        }
        let inner = Arc::new(Inner {
            tripped: AtomicU8::new(0),
            observed: AtomicU64::new(0),
            unreadable: AtomicBool::new(false),
            started: Instant::now(),
            done: Mutex::new(false),
            wake: Condvar::new(),
        });
        let (time, memory) = (self.time, self.memory);
        let watched = inner.clone();
        let thread = std::thread::Builder::new()
            .name("dive-budget".into())
            .spawn(move || watch(&watched, time, memory))
            .ok()?;
        Some(Watchdog {
            budget: Self {
                time,
                memory,
                signal: Some(inner),
            },
            thread: Some(thread),
        })
    }

    /// What is exhausted, if anything. One relaxed load while the budget
    /// holds; `None` for an unarmed budget.
    #[inline]
    #[must_use]
    pub fn poll(&self) -> Option<Exhausted> {
        let inner = self.signal.as_ref()?;
        if inner.tripped.load(Ordering::Relaxed) == 0 {
            return None;
        }
        let resource = match inner.tripped.load(Ordering::Acquire) {
            1 => Resource::Time,
            _ => Resource::Memory,
        };
        let limit = match resource {
            Resource::Time => self
                .time
                .map_or(0, |t| u64::try_from(t.as_millis()).unwrap_or(u64::MAX)),
            Resource::Memory => self.memory.unwrap_or(0),
        };
        Some(Exhausted {
            resource,
            limit,
            observed: inner.observed.load(Ordering::Relaxed),
            elapsed_ms: millis(inner.started.elapsed()),
        })
    }

    /// Whether the armed budget's watchdog failed to read memory, so that its
    /// memory ceiling was not enforced.
    #[must_use]
    pub fn memory_unavailable(&self) -> bool {
        self.memory.is_some()
            && self
                .signal
                .as_ref()
                .is_some_and(|inner| inner.unreadable.load(Ordering::Relaxed))
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// `opts` with its budget armed, and the watchdog to keep alive for the run;
/// `opts` itself when the budget is unlimited or already armed. Entry points
/// that run several stages arm once, so the stages share one clock.
///
/// ```
/// use std::time::Duration;
/// use dive_check::{CheckOptions, budget::{Budget, arm}};
/// let opts = CheckOptions {
///     budget: Budget::new(Some(Duration::from_secs(60)), None),
///     ..CheckOptions::dive(0)
/// };
/// let (watchdog, armed) = arm(&opts);
/// assert!(watchdog.is_some() && armed.budget.is_armed());
/// let (again, _) = arm(&armed);
/// assert!(again.is_none());
/// ```
#[must_use]
pub fn arm(
    opts: &crate::CheckOptions,
) -> (Option<Watchdog>, std::borrow::Cow<'_, crate::CheckOptions>) {
    match opts.budget.start() {
        Some(watchdog) => {
            let armed = crate::CheckOptions {
                budget: watchdog.budget().clone(),
                ..opts.clone()
            };
            (Some(watchdog), std::borrow::Cow::Owned(armed))
        }
        None => (None, std::borrow::Cow::Borrowed(opts)),
    }
}

/// The armed budget's watchdog thread; stops it when dropped.
#[derive(Debug)]
pub struct Watchdog {
    budget: Budget,
    thread: Option<JoinHandle<()>>,
}

impl Watchdog {
    /// The armed budget, to hand to the run.
    #[must_use]
    pub const fn budget(&self) -> &Budget {
        &self.budget
    }
}

impl Drop for Watchdog {
    /// Stops the watchdog. A sample in flight is not waited for: the thread
    /// sees the stop when the sample returns and exits by itself.
    fn drop(&mut self) {
        if let Some(inner) = &self.budget.signal {
            *inner.done.lock().unwrap_or_else(PoisonError::into_inner) = true;
            inner.wake.notify_all();
        }
        if let Some(thread) = self.thread.take()
            && thread.is_finished()
        {
            let _ = thread.join();
        }
    }
}

fn watch(inner: &Inner, time: Option<Duration>, memory: Option<u64>) {
    let started = inner.started;
    // The first sample waits a full period: short runs never sample.
    let mut sampled = started;
    let tick = time.map_or(TICK, |t| (t / 10).clamp(Duration::from_millis(1), TICK));
    let mut done = inner.done.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if *done || inner.tripped.load(Ordering::Relaxed) != 0 {
            return;
        }
        let elapsed = started.elapsed();
        if let Some(limit) = time
            && elapsed >= limit
        {
            inner.trip(Resource::Time, millis(elapsed));
            return;
        }
        if let Some(ceiling) = memory
            && !inner.unreadable.load(Ordering::Relaxed)
            && sampled.elapsed() >= MEMORY_EVERY
        {
            sampled = Instant::now();
            drop(done);
            let rss = resident_bytes();
            done = inner.done.lock().unwrap_or_else(PoisonError::into_inner);
            match rss {
                Some(rss) if rss > ceiling => {
                    inner.trip(Resource::Memory, rss);
                    return;
                }
                Some(_) => {}
                // Never claim a ceiling that cannot be measured.
                None => inner.unreadable.store(true, Ordering::Relaxed),
            }
        }
        done = inner
            .wake
            .wait_timeout(done, tick)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

/// The process's memory, where the platform reports it: resident plus
/// swapped pages on Linux (a process that swaps still counts), resident pages
/// on macOS.
#[must_use]
pub fn resident_bytes() -> Option<u64> {
    if cfg!(target_os = "linux") {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        let rss = kib_field(&text, "VmRSS:")?;
        rss.checked_add(kib_field(&text, "VmSwap:").unwrap_or(0))
    } else if cfg!(target_os = "macos") {
        let pid = std::process::id().to_string();
        command_number(Command::new("/bin/ps").args(["-o", "rss=", "-p", &pid]))
            .and_then(|kib| kib.checked_mul(1024))
    } else {
        None
    }
}

/// Whether this platform can read the process's memory, checked once per
/// process without starting another process. A ceiling is claimed only
/// where it is.
#[must_use]
pub fn memory_readable() -> bool {
    static READABLE: OnceLock<bool> = OnceLock::new();
    *READABLE.get_or_init(|| {
        if cfg!(target_os = "linux") {
            std::fs::read_to_string("/proc/self/status")
                .is_ok_and(|text| kib_field(&text, "VmRSS:").is_some())
        } else if cfg!(target_os = "macos") {
            std::path::Path::new("/bin/ps").exists()
        } else {
            false
        }
    })
}

/// The machine's physical memory, where the platform reports it; read once
/// per process.
#[must_use]
pub fn physical_bytes() -> Option<u64> {
    static PHYSICAL: OnceLock<Option<u64>> = OnceLock::new();
    *PHYSICAL.get_or_init(|| {
        if cfg!(target_os = "linux") {
            std::fs::read_to_string("/proc/meminfo")
                .ok()
                .and_then(|text| kib_field(&text, "MemTotal:"))
        } else if cfg!(target_os = "macos") {
            command_number(Command::new("/usr/sbin/sysctl").args(["-n", "hw.memsize"]))
        } else {
            None
        }
    })
}

/// The default memory ceiling, `--max-memory auto`: half of physical memory.
/// TLC's default footprint is its JVM heap (a quarter of physical memory)
/// plus off-heap fingerprint storage and its disk queue; dive keeps all of
/// that resident, so half keeps dive's capacity at least TLC's while still
/// stopping a runaway run before it exhausts the machine (ADR 0107). `None`
/// where physical memory is unknown or the process's memory cannot be read.
///
/// ```
/// if let Some(bytes) = dive_check::budget::default_memory() {
///     assert!(bytes > 0);
/// }
/// ```
#[must_use]
pub fn default_memory() -> Option<u64> {
    physical_bytes()
        .filter(|_| memory_readable())
        .map(|bytes| bytes / 2)
}

fn kib_field(text: &str, key: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with(key))?;
    let kib: u64 = line
        .strip_prefix(key)?
        .trim()
        .strip_suffix("kB")?
        .trim()
        .parse()
        .ok()?;
    kib.checked_mul(1024)
}

fn command_number(command: &mut Command) -> Option<u64> {
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "watchdog controls")]
    use super::*;

    fn wait_for(budget: &Budget) -> Option<Exhausted> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if let Some(e) = budget.poll() {
                return Some(e);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn an_expired_time_limit_trips_with_the_elapsed_time() {
        let watchdog = Budget::new(Some(Duration::from_millis(20)), None)
            .start()
            .expect("watchdog");
        let e = wait_for(watchdog.budget()).expect("trips");
        assert_eq!(e.resource, Resource::Time);
        assert_eq!(e.limit, 20);
        assert!(e.observed >= 20);
    }

    #[test]
    fn a_memory_ceiling_below_the_footprint_trips_where_memory_is_known() {
        if resident_bytes().is_none() {
            return;
        }
        let watchdog = Budget::new(None, Some(1)).start().expect("watchdog");
        let e = wait_for(watchdog.budget()).expect("trips");
        assert_eq!((e.resource, e.limit), (Resource::Memory, 1));
        assert!(e.observed > 1);
    }

    #[test]
    fn generous_limits_and_unarmed_budgets_never_trip() {
        let watchdog = Budget::new(Some(Duration::from_secs(3600)), Some(u64::MAX))
            .start()
            .expect("watchdog");
        std::thread::sleep(Duration::from_millis(150));
        assert!(watchdog.budget().poll().is_none());
        assert!(Budget::new(Some(Duration::ZERO), None).poll().is_none());
        assert!(Budget::unlimited().start().is_none());
        // An armed budget is not armed twice.
        assert!(watchdog.budget().start().is_none());
    }

    #[test]
    fn the_platform_reports_memory_on_linux_and_macos() {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(resident_bytes().is_some_and(|b| b > 0));
            assert!(physical_bytes().is_some_and(|b| b > resident_bytes().unwrap_or(0)));
        }
    }
}
