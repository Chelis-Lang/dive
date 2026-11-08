//! Run budgets (ADR 0107): a wall-clock limit and a whole-process
//! resident-memory ceiling, the runaway guards that replace fixed counts.
//!
//! A [`Budget`] says how long a run may take and how much resident memory the
//! process may hold. [`Budget::start`] arms it: a watchdog thread measures
//! elapsed time and the resident set size, and trips a shared flag once
//! either is exceeded. Engines read the flag at points they already pass
//! (each expanded state, each claimed block, each analysis unit) with one
//! relaxed atomic load on the normal path. A
//! tripped budget stops the run with [`crate::Verdict::Stopped`]: never a
//! verdict about the model, always naming the resource, its limit, what was
//! observed and the phase the run was in.
//!
//! TLC has no such counts. It is bounded by its JVM heap (out of memory is
//! `SYSTEM_OUT_OF_MEMORY`, exit 1 or 255) and has no wall-clock limit except
//! the developer property `-Dtlc2.TLC.stopAfter`. Under `--compat=tlc` the
//! time limit is therefore unset; the memory ceiling stands for TLC's heap.
//!
//! Memory is sampled when a finite-memory budget is armed, periodically,
//! and before its owner publishes a result. Linux includes resident and
//! swapped pages from `/proc/self/status`; macOS uses a safe native Mach
//! resident-memory query (ADR 0109). Sampling is not an allocation-time hard
//! cap and cannot observe every transient peak. Unavailable observations
//! fail closed with [`MeasurementError`], never zero bytes or a fake stop.
//! Nested entry points reuse the armed signal; only the watchdog owner calls
//! [`Watchdog::finish`]. Physical RAM lookup for the default ceiling is cached
//! per process, and an unavailable default is an error in the file API.
//!
//! ```
//! use std::time::Duration;
//! use dive_runtime::budget::Budget;
//! let budget = Budget::new(Some(Duration::from_secs(3600)), None);
//! let watchdog = budget.start().expect("watchdog thread starts").expect("a limited budget starts a watchdog");
//! assert!(watchdog.budget().poll().expect("measurement").is_none());
//! assert!(Budget::unlimited().start().expect("unlimited").is_none());
//! ```

use std::process::Command;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Stable code for failure to start a requested budget's watchdog.
pub const START_FAILURE_CODE: &str = "DIVE-R0001";

/// Stable code for an unavailable memory-budget measurement.
pub const MEASUREMENT_FAILURE_CODE: &str = "DIVE-R0002";

/// The boundary at which a process-memory observation was requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplePoint {
    /// Before checking starts.
    Initial,
    /// A periodic watchdog observation.
    Periodic,
    /// Before the owning entry point publishes a result.
    Final,
}
impl SamplePoint {
    /// Stable report spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Periodic => "periodic",
            Self::Final => "final",
        }
    }
    const fn flag(self) -> u8 {
        match self {
            Self::Initial => 1,
            Self::Periodic => 2,
            Self::Final => 3,
        }
    }
    fn from_flag(flag: u8) -> Self {
        match flag {
            1 => Self::Initial,
            3 => Self::Final,
            _ => Self::Periodic,
        }
    }
}

/// A requested memory observation is unavailable, not an exhausted resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasurementError {
    /// Where observation failed.
    pub point: SamplePoint,
    /// The failed measurement, without inventing an observed byte count.
    pub kind: MeasurementKind,
}
/// The unavailable information needed to enforce memory policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementKind {
    /// Current whole-process resident memory.
    Process,
    /// Physical RAM needed to resolve the default ceiling.
    DefaultCeiling,
}
impl MeasurementKind {
    /// Stable report spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Process => "process-memory",
            Self::DefaultCeiling => "default-ceiling",
        }
    }
}

impl MeasurementError {
    /// An unavailable default ceiling because physical RAM could not be read.
    #[must_use]
    pub const fn default_ceiling_unavailable() -> Self {
        Self {
            point: SamplePoint::Initial,
            kind: MeasurementKind::DefaultCeiling,
        }
    }

    /// An unavailable process RSS observation.
    #[must_use]
    pub const fn unavailable(point: SamplePoint) -> Self {
        Self {
            point,
            kind: MeasurementKind::Process,
        }
    }
}
impl std::fmt::Display for MeasurementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{MEASUREMENT_FAILURE_CODE}: {} at the {} sample; retry on a supported host or pass --max-memory none",
            match self.kind {
                MeasurementKind::Process => "cannot measure the current process's memory",
                MeasurementKind::DefaultCeiling =>
                    "cannot determine the default memory ceiling because physical RAM is unavailable",
            },
            self.point.name()
        )
    }
}
impl std::error::Error for MeasurementError {}

/// The requested time/memory budget could not be armed. No check may run
/// with that budget silently disabled.
#[derive(Debug)]
pub enum StartError {
    /// The watchdog thread could not start.
    Thread(std::io::Error),
    /// The initial memory observation was unavailable.
    Measurement(MeasurementError),
}
impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Thread(error) => write!(
                f,
                "{START_FAILURE_CODE}: cannot start the budget watchdog: {error}; free host resources and retry, or pass --time-limit none --max-memory none"
            ),
            Self::Measurement(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Thread(error) => error,
            Self::Measurement(error) => error,
        })
    }
}

/// Actual memory observations; their maximum is not the true process peak.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemorySamples {
    /// Number of successful observations.
    pub count: u64,
    /// Largest observed byte count, absent when no observation succeeded.
    pub maximum: Option<u64>,
    /// Whether the initial observation succeeded.
    pub initial: bool,
    /// Whether the terminal observation succeeded.
    pub final_sample: bool,
    /// First unavailable observation, if any.
    pub unavailable: Option<SamplePoint>,
}

/// The dive profile's default wall-clock limit. Provisional: the release
/// value is chosen from the benchmark campaign before release (ADR 0107).
/// The harness and the gate pass explicit limits; library tests that keep
/// the default finish far below it, and no test depends on its value.
pub const DEFAULT_TIME_LIMIT: Duration = Duration::from_mins(30);

/// How often the watchdog wakes to compare elapsed time with the limit.
const TICK: Duration = Duration::from_millis(50);

/// Period between background memory samples, in addition to the initial and
/// terminal observations. Boundary sampling uses no child process on macOS.
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
    /// Discovered in-model state identities.
    States,
    /// Breadth-first discovery depth (initial states have depth one).
    Depth,
}

impl Resource {
    /// The name in dive's output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Memory => "memory",
            Self::States => "states",
            Self::Depth => "depth",
        }
    }

    const fn flag(self) -> u8 {
        match self {
            Self::Time => 1,
            Self::Memory => 2,
            Self::States => 3,
            Self::Depth => 4,
        }
    }
}

/// The phase a stopped run was in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Model startup, before state generation starts.
    Startup,
    /// Evaluator-backed startup work while binding a fresh model.
    Binding,
    /// The owning entry point samples resources before publishing a result.
    Finalization,
    /// Safety search.
    Search,
    /// Finite sampled safety execution.
    Simulation,
    /// Depth-first iterative-deepening execution.
    Dfid,
    /// Behavior-graph capture for temporal checking.
    Capture,
    /// Temporal analysis after capture: planning, labelling, product, search.
    Temporal,
    /// Independent validation of supplied counterexample states.
    Validation,
}

impl Phase {
    /// The name in dive's output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Startup => "check-startup",
            Self::Binding => "model-binding",
            Self::Finalization => "finalization",
            Self::Search => "safety-search",
            Self::Simulation => "simulation",
            Self::Dfid => "dfid",
            Self::Capture => "graph-capture",
            Self::Temporal => "temporal-analysis",
            Self::Validation => "trace-validation",
        }
    }
}

/// A run stopped by its budget. The statistics at the stop are the result's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stopped {
    /// What ran out.
    pub resource: Resource,
    /// The limit: milliseconds, bytes, admitted states or BFS levels.
    pub limit: u64,
    /// The exhausted observation in the same unit. For state/depth ceilings,
    /// this is the next admission that was prevented.
    pub observed: u64,
    /// Milliseconds from arming the relevant budget to the reported stop.
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
            Resource::States => {
                "raise --max-states, or pass --max-states=none; the run made no claim about the model"
            }
            Resource::Depth => {
                "raise --max-depth, or pass --max-depth=none; the run made no claim about the model"
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
            Resource::States | Resource::Depth => write!(
                f,
                "stopped during {}: the next {} would be {}, exceeding the limit of {}",
                self.phase.name(),
                self.resource.name(),
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

const MEASUREMENT_FAILED: u8 = 5;
const PUBLISHING: u8 = 6;

#[derive(Default)]
enum Sampler {
    #[default]
    Native,
    #[cfg(test)]
    Injected(Arc<dyn Fn() -> Option<u64> + Send + Sync>),
}
impl std::fmt::Debug for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("process-memory sampler")
    }
}
impl Sampler {
    fn read(&self) -> Option<u64> {
        match self {
            Self::Native => resident_bytes(),
            #[cfg(test)]
            Self::Injected(sample) => sample(),
        }
    }
}

#[derive(Debug)]
struct Inner {
    tripped: AtomicU8,
    observed: AtomicU64,
    unavailable: AtomicU8,
    sample_count: AtomicU64,
    sample_maximum: AtomicU64,
    sample_points: AtomicU8,
    sampler: Sampler,
    started: Instant,
    done: Mutex<bool>,
    wake: Condvar,
}
impl Inner {
    fn publish(&self, flag: u8, observed: u64) {
        if self
            .tripped
            .compare_exchange(0, PUBLISHING, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            self.observed.store(observed, Ordering::Relaxed);
            self.tripped.store(flag, Ordering::Release);
        }
    }
    fn trip(&self, resource: Resource, observed: u64) {
        self.publish(resource.flag(), observed);
    }
    fn record_sample(
        &self,
        point: SamplePoint,
        value: Option<u64>,
        ceiling: u64,
    ) -> Result<(), MeasurementError> {
        let Some(bytes) = value else {
            let _ = self.unavailable.compare_exchange(
                0,
                point.flag(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
            self.publish(MEASUREMENT_FAILED, 0);
            return Err(MeasurementError::unavailable(point));
        };
        self.sample_maximum.fetch_max(bytes, Ordering::Relaxed);
        self.sample_count.fetch_add(1, Ordering::Relaxed);
        self.sample_points
            .fetch_or(1 << point.flag(), Ordering::Release);
        if bytes > ceiling {
            self.trip(Resource::Memory, bytes);
        }
        Ok(())
    }
    fn sample(&self, point: SamplePoint, ceiling: u64) -> Result<(), MeasurementError> {
        self.record_sample(point, self.sampler.read(), ceiling)
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
    /// Trips an armed budget deterministically while a test expansion is in
    /// flight (this crate's tests only).
    #[cfg(test)]
    #[doc(hidden)]
    pub fn trip_for_test(&self) {
        if let Some(inner) = &self.signal {
            inner.trip(Resource::Time, millis(inner.started.elapsed()));
        }
    }

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
    /// watchdog stops when dropped. `None` means unlimited or already armed.
    ///
    /// # Errors
    /// Returns [`StartError`] when the initial memory observation is unavailable
    /// or the thread cannot start. Callers must abort rather than disable limits.
    pub fn start(&self) -> Result<Option<Watchdog>, StartError> {
        self.start_with_sampler(
            |watched, time, memory| {
                std::thread::Builder::new()
                    .name("dive-budget".into())
                    .spawn(move || watch(&watched, time, memory))
            },
            Sampler::Native,
        )
    }

    #[cfg(test)]
    fn start_with(
        &self,
        spawn: impl FnOnce(Arc<Inner>, Option<Duration>, Option<u64>) -> std::io::Result<JoinHandle<()>>,
    ) -> Result<Option<Watchdog>, StartError> {
        self.start_with_sampler(spawn, Sampler::Native)
    }

    fn start_with_sampler(
        &self,
        spawn: impl FnOnce(Arc<Inner>, Option<Duration>, Option<u64>) -> std::io::Result<JoinHandle<()>>,
        sampler: Sampler,
    ) -> Result<Option<Watchdog>, StartError> {
        if self.is_unlimited() || self.is_armed() {
            return Ok(None);
        }
        let inner = Arc::new(Inner {
            tripped: AtomicU8::new(0),
            observed: AtomicU64::new(0),
            unavailable: AtomicU8::new(0),
            sample_count: AtomicU64::new(0),
            sample_maximum: AtomicU64::new(0),
            sample_points: AtomicU8::new(0),
            sampler,
            started: Instant::now(),
            done: Mutex::new(false),
            wake: Condvar::new(),
        });
        let (time, memory) = (self.time, self.memory);
        if time == Some(Duration::ZERO) {
            inner.trip(Resource::Time, millis(inner.started.elapsed()));
        }
        if let Some(ceiling) = memory {
            inner
                .sample(SamplePoint::Initial, ceiling)
                .map_err(StartError::Measurement)?;
        }
        let thread = spawn(inner.clone(), time, memory).map_err(StartError::Thread)?;
        Ok(Some(Watchdog {
            budget: Self {
                time,
                memory,
                signal: Some(inner),
            },
            thread: Some(thread),
            finished: false,
        }))
    }

    /// What is exhausted, if anything. One relaxed load while the budget
    /// holds; `Ok(None)` for an unarmed budget.
    ///
    /// # Errors
    /// Returns the first unavailable process-memory observation. It is never
    /// converted into a resource exhaustion with an invented byte count.
    #[inline]
    pub fn poll(&self) -> Result<Option<Exhausted>, MeasurementError> {
        let Some(inner) = &self.signal else {
            return Ok(None);
        };
        let mut flag = inner.tripped.load(Ordering::Relaxed);
        if flag == 0 {
            return Ok(None);
        }
        flag = inner.tripped.load(Ordering::Acquire);
        while flag == PUBLISHING {
            std::hint::spin_loop();
            flag = inner.tripped.load(Ordering::Acquire);
        }
        if flag == MEASUREMENT_FAILED {
            return Err(MeasurementError::unavailable(SamplePoint::from_flag(
                inner.unavailable.load(Ordering::Relaxed),
            )));
        }
        let resource = if flag == 1 {
            Resource::Time
        } else {
            Resource::Memory
        };
        let limit = match resource {
            Resource::Time => self
                .time
                .map_or(0, |t| u64::try_from(t.as_millis()).unwrap_or(u64::MAX)),
            Resource::Memory => self.memory.unwrap_or(0),
            Resource::States | Resource::Depth => 0,
        };
        Ok(Some(Exhausted {
            resource,
            limit,
            observed: inner.observed.load(Ordering::Relaxed),
            elapsed_ms: millis(inner.started.elapsed()),
        }))
    }

    /// Actual memory samples of this armed run, if it has been started.
    #[must_use]
    pub fn memory_samples(&self) -> Option<MemorySamples> {
        let inner = self.signal.as_ref()?;
        let points = inner.sample_points.load(Ordering::Acquire);
        let count = inner.sample_count.load(Ordering::Relaxed);
        let unavailable = inner.unavailable.load(Ordering::Relaxed);
        Some(MemorySamples {
            count,
            maximum: (count != 0).then(|| inner.sample_maximum.load(Ordering::Relaxed)),
            initial: points & (1 << 1) != 0,
            final_sample: points & (1 << 3) != 0,
            unavailable: (unavailable != 0).then(|| SamplePoint::from_flag(unavailable)),
        })
    }
}

/// Applies the owning watchdog's terminal observation to a safety result.
/// Genuine earlier errors and witnesses retain their precedence; only a
/// successful exhaustion is replaced by a resource stop or measurement error.
pub fn finish_result(
    result: &mut crate::CheckResult,
    finished: Result<Option<Exhausted>, MeasurementError>,
    phase: Phase,
) {
    if result.verdict != crate::Verdict::Success {
        return;
    }
    result.verdict = match finished {
        Ok(Some(exhausted)) => crate::Verdict::Stopped(Box::new(exhausted.during(phase))),
        Err(error) => crate::Verdict::BudgetMeasurement { error },
        Ok(None) => return,
    };
    result.counterexample = None;
    result.collision = None;
    result.tlc_prints_stats = false;
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
/// use dive_runtime::{CheckOptions, budget::{Budget, arm}};
/// let opts = CheckOptions {
///     budget: Budget::new(Some(Duration::from_secs(60)), None),
///     ..CheckOptions::dive(0)
/// };
/// let (watchdog, armed) = arm(&opts).expect("watchdog thread starts");
/// assert!(watchdog.is_some() && armed.budget.is_armed());
/// let (again, _) = arm(&armed).expect("already armed");
/// assert!(again.is_none());
/// ```
///
/// # Errors
/// Returns [`StartError`] if a requested budget cannot be enforced.
pub fn arm(
    opts: &crate::CheckOptions,
) -> Result<(Option<Watchdog>, std::borrow::Cow<'_, crate::CheckOptions>), StartError> {
    match opts.budget.start()? {
        Some(watchdog) => {
            let armed = crate::CheckOptions {
                budget: watchdog.budget().clone(),
                ..opts.clone()
            };
            Ok((Some(watchdog), std::borrow::Cow::Owned(armed)))
        }
        None => Ok((None, std::borrow::Cow::Borrowed(opts))),
    }
}

/// The armed budget's watchdog thread; stops it when dropped.
#[derive(Debug)]
pub struct Watchdog {
    budget: Budget,
    thread: Option<JoinHandle<()>>,
    finished: bool,
}

impl Watchdog {
    /// The armed budget, to hand to the run.
    #[must_use]
    pub const fn budget(&self) -> &Budget {
        &self.budget
    }

    /// Checks the clock and takes the final memory sample before publishing a result. Only the
    /// watchdog's owning entry point calls this; nested armed calls do not.
    ///
    /// # Errors
    /// Returns a typed failure when memory measurement is unavailable.
    pub fn finish(&mut self) -> Result<Option<Exhausted>, MeasurementError> {
        if self.finished {
            return self.budget.poll();
        }
        self.finished = true;
        let earlier = self.budget.poll();
        if let Some(inner) = &self.budget.signal {
            *inner.done.lock().unwrap_or_else(PoisonError::into_inner) = true;
            inner.wake.notify_all();
            if let Some(ceiling) = self.budget.memory {
                let _ = inner.sample(SamplePoint::Final, ceiling);
            }
            if let Some(limit) = self.budget.time {
                let elapsed = inner.started.elapsed();
                if elapsed >= limit {
                    inner.trip(Resource::Time, millis(elapsed));
                }
            }
        }
        match earlier {
            Ok(None) => self.budget.poll(),
            prior => prior,
        }
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
    // Initial sampling completed before spawning; background sampling waits
    // a full period. The owner supplies the terminal observation.
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
            && sampled.elapsed() >= MEMORY_EVERY
        {
            sampled = Instant::now();
            drop(done);
            let rss = inner.sampler.read();
            done = inner.done.lock().unwrap_or_else(PoisonError::into_inner);
            if *done {
                return;
            }
            if inner
                .record_sample(SamplePoint::Periodic, rss, ceiling)
                .is_err()
            {
                return;
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
        linux_process_bytes(&text)
    } else {
        native_resident_bytes()
    }
}

#[cfg(target_os = "macos")]
fn native_resident_bytes() -> Option<u64> {
    memory_stats::memory_stats().and_then(|sample| u64::try_from(sample.physical_mem).ok())
}
#[cfg(not(target_os = "macos"))]
fn native_resident_bytes() -> Option<u64> {
    None
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
/// where physical memory is unknown. A finite budget validates process-memory
/// availability independently when it is armed.
///
/// ```
/// if let Some(bytes) = dive_runtime::budget::default_memory() {
///     assert!(bytes > 0);
/// }
/// ```
#[must_use]
pub fn default_memory() -> Option<u64> {
    physical_bytes()
        .map(|bytes| bytes / 2)
        .filter(|bytes| *bytes != 0)
}

fn linux_process_bytes(text: &str) -> Option<u64> {
    kib_field(text, "VmRSS:")?.checked_add(kib_field(text, "VmSwap:")?)
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
            if let Some(e) = budget.poll().expect("measurement") {
                return Some(e);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn a_spawn_failure_is_distinct_from_an_unlimited_budget() {
        let limited = Budget::new(Some(Duration::from_secs(60)), None);
        let error = limited
            .start_with(|_, _, _| Err(std::io::Error::other("injected thread exhaustion")))
            .expect_err("a requested watchdog must not disappear");
        assert!(error.to_string().contains(START_FAILURE_CODE));
        assert!(error.to_string().contains("injected thread exhaustion"));
        assert!(
            error
                .to_string()
                .contains("--time-limit none --max-memory none")
        );
        assert!(std::error::Error::source(&error).is_some());
        assert!(!limited.is_armed());
        assert!(!limited.is_unlimited());

        let attempted = std::cell::Cell::new(false);
        let unlimited = Budget::unlimited()
            .start_with(|_, _, _| {
                attempted.set(true);
                Err(std::io::Error::other(
                    "must not attempt an unlimited watchdog",
                ))
            })
            .expect("unlimited needs no host resource");
        assert!(unlimited.is_none());
        assert!(!attempted.get());
    }

    #[test]
    fn an_expired_time_limit_trips_with_the_elapsed_time() {
        let watchdog = Budget::new(Some(Duration::from_millis(20)), None)
            .start()
            .expect("watchdog thread starts")
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
        let watchdog = Budget::new(None, Some(1))
            .start()
            .expect("watchdog thread starts")
            .expect("watchdog");
        let e = wait_for(watchdog.budget()).expect("trips");
        assert_eq!((e.resource, e.limit), (Resource::Memory, 1));
        assert!(e.observed > 1);
    }

    #[test]
    fn generous_limits_and_unarmed_budgets_never_trip() {
        let watchdog = Budget::new(Some(Duration::from_secs(3600)), Some(u64::MAX))
            .start()
            .expect("watchdog thread starts")
            .expect("watchdog");
        std::thread::sleep(Duration::from_millis(150));
        assert!(watchdog.budget().poll().expect("measurement").is_none());
        assert!(
            Budget::new(Some(Duration::ZERO), None)
                .poll()
                .expect("measurement")
                .is_none()
        );
        assert!(Budget::unlimited().start().expect("unlimited").is_none());
        // An armed budget is not armed twice.
        assert!(watchdog.budget().start().expect("armed").is_none());
    }

    #[test]
    fn linux_memory_requires_both_checked_components() {
        assert_eq!(
            linux_process_bytes("VmRSS: 2 kB\nVmSwap: 3 kB\n"),
            Some(5 * 1024)
        );
        for text in [
            "VmRSS: 2 kB\n",
            "VmRSS: 2 kB\nVmSwap: bad kB\n",
            "VmRSS: 2 kB\nVmSwap: 18446744073709551615 kB\n",
            "VmRSS: 18014398509481983 kB\nVmSwap: 1 kB\n",
        ] {
            assert_eq!(linux_process_bytes(text), None, "{text}");
        }
    }

    #[test]
    fn the_platform_reports_memory_on_linux_and_macos() {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(resident_bytes().is_some_and(|b| b > 0));
            assert!(physical_bytes().is_some_and(|b| b > resident_bytes().unwrap_or(0)));
        }
    }
    fn injected(sample: impl Fn() -> Option<u64> + Send + Sync + 'static) -> Watchdog {
        Budget::new(None, Some(100))
            .start_with_sampler(
                |inner, time, memory| {
                    std::thread::Builder::new().spawn(move || watch(&inner, time, memory))
                },
                Sampler::Injected(Arc::new(sample)),
            )
            .expect("initial observation succeeds")
            .expect("finite budget")
    }

    #[test]
    fn an_initial_measurement_failure_never_starts_an_unguarded_run() {
        let attempted = std::cell::Cell::new(false);
        let error = Budget::new(None, Some(100))
            .start_with_sampler(
                |_, _, _| {
                    attempted.set(true);
                    Err(std::io::Error::other("must not spawn"))
                },
                Sampler::Injected(Arc::new(|| None)),
            )
            .expect_err("observation must fail closed");
        assert!(matches!(
            error,
            StartError::Measurement(MeasurementError {
                point: SamplePoint::Initial,
                kind: MeasurementKind::Process
            })
        ));
        assert!(!attempted.get());
        assert!(error.to_string().contains(MEASUREMENT_FAILURE_CODE));
    }

    #[test]
    fn initial_and_final_samples_are_shared_and_finish_is_idempotent() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let reads = calls.clone();
        let mut watchdog = injected(move || {
            Some(if reads.fetch_add(1, Ordering::Relaxed) == 0 {
                10
            } else {
                20
            })
        });
        assert!(watchdog.budget().start().expect("already armed").is_none());
        assert_eq!(watchdog.finish(), Ok(None));
        assert_eq!(watchdog.finish(), Ok(None));
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(
            watchdog.budget().memory_samples(),
            Some(MemorySamples {
                count: 2,
                maximum: Some(20),
                initial: true,
                final_sample: true,
                unavailable: None
            })
        );
    }

    #[test]
    fn final_unknown_memory_is_a_typed_error_never_a_zero_byte_stop() {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut watchdog =
            injected(move || (calls.fetch_add(1, Ordering::Relaxed) == 0).then_some(10));
        assert_eq!(
            watchdog.finish(),
            Err(MeasurementError::unavailable(SamplePoint::Final))
        );
        assert_eq!(
            watchdog.budget().poll(),
            Err(MeasurementError::unavailable(SamplePoint::Final))
        );
        assert_eq!(
            watchdog.budget().memory_samples(),
            Some(MemorySamples {
                count: 1,
                maximum: Some(10),
                initial: true,
                final_sample: false,
                unavailable: Some(SamplePoint::Final)
            })
        );
    }

    #[test]
    fn final_excess_has_the_real_observed_bytes() {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut watchdog = injected(move || {
            Some(if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                10
            } else {
                101
            })
        });
        let stopped = watchdog.finish().expect("measurement").expect("excess");
        assert_eq!(
            (stopped.resource, stopped.limit, stopped.observed),
            (Resource::Memory, 100, 101)
        );
        assert_eq!(
            watchdog.budget().memory_samples().expect("armed").maximum,
            Some(101)
        );
    }

    #[test]
    fn a_periodic_sensor_failure_interrupts_without_fabricating_exhaustion() {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut watchdog =
            injected(move || (calls.fetch_add(1, Ordering::Relaxed) == 0).then_some(10));
        let started = Instant::now();
        while watchdog.budget().poll() == Ok(None) && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let expected = Err(MeasurementError::unavailable(SamplePoint::Periodic));
        assert_eq!(watchdog.budget().poll(), expected);
        assert_eq!(
            watchdog.finish(),
            expected,
            "retain the first observed failure"
        );
    }

    #[test]
    fn zero_time_is_known_before_the_watchdog_can_run() {
        let mut watchdog = Budget::new(Some(Duration::ZERO), None)
            .start_with(|_, _, _| std::thread::Builder::new().spawn(|| {}))
            .expect("spawn")
            .expect("finite");
        let stop = watchdog
            .budget()
            .poll()
            .expect("clock")
            .expect("zero budget");
        assert_eq!((stop.resource, stop.limit), (Resource::Time, 0));
        assert_eq!(
            watchdog.finish().expect("clock").expect("stop").resource,
            Resource::Time
        );
    }

    #[test]
    fn final_clock_check_does_not_depend_on_watchdog_scheduling() {
        let mut watchdog = Budget::new(Some(Duration::from_millis(1)), None)
            .start_with(|_, _, _| std::thread::Builder::new().spawn(|| {}))
            .expect("spawn")
            .expect("finite");
        std::thread::sleep(Duration::from_millis(3));
        assert_eq!(
            watchdog.budget().poll(),
            Ok(None),
            "injected watchdog performs no checks"
        );
        let stop = watchdog.finish().expect("clock").expect("deadline");
        assert_eq!((stop.resource, stop.limit), (Resource::Time, 1));
        assert!(stop.observed >= 1);
    }
}

/// The run evidence used to classify cooperative evaluator interruption.
///
/// This is passed explicitly through checker classification helpers. Its opaque
/// identity prevents another driver's interruption becoming this run's stop.
#[derive(Debug)]
pub struct EvaluationControl {
    budget: Budget,
    phase: Phase,
    identity: u64,
}

impl dive_eval::HostControl for EvaluationControl {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        match self.budget.poll() {
            Ok(None) => Ok(()),
            Ok(Some(_)) | Err(_) => Err(dive_eval::Interrupted::new(self.identity)),
        }
    }
}

/// Checks producer assumptions under the already armed startup budget.
/// Binding and independent validation retain their own explicit boundaries.
#[doc(hidden)]
#[must_use]
pub fn check_assumptions(model: &dive_model::Model, budget: &Budget) -> Option<crate::Verdict> {
    let control = match EvaluationControl::new(budget, Phase::Startup) {
        Ok(control) => control,
        Err(error) => {
            return Some(crate::Verdict::InternalError {
                message: error.to_string(),
            });
        }
    };
    let _scope = match control.enter(&model.evaluator) {
        Ok(scope) => scope,
        Err(error) => {
            return Some(crate::Verdict::InternalError {
                message: error.to_string(),
            });
        }
    };
    match model.check_assumptions() {
        dive_eval::AssumptionOutcome::AllTrue => None,
        dive_eval::AssumptionOutcome::False {
            property, message, ..
        } => Some(crate::Verdict::AssumptionFalse { property, message }),
        dive_eval::AssumptionOutcome::Error { error, message, .. } => Some(
            control
                .verdict(&error)
                .unwrap_or_else(|| crate::class::assumption_error(&error, message)),
        ),
    }
}

impl EvaluationControl {
    /// Makes an explicit context from the already armed budget and actual phase.
    ///
    /// # Errors
    /// The native identity representation is exhausted.
    pub fn new(budget: &Budget, phase: Phase) -> Result<Arc<Self>, dive_eval::HostControlConflict> {
        // Pointer identity is opaque and never dereferenced. Retaining this
        // budget retains its signal, so another live run cannot reuse it.
        let identity = budget
            .signal
            .as_ref()
            .map_or(Ok(0), |signal| u64::try_from(Arc::as_ptr(signal) as usize))
            .map_err(|_| dive_eval::HostControlConflict)?;
        Ok(Arc::new(Self {
            budget: budget.clone(),
            phase,
            identity,
        }))
    }

    /// Installs this phase's context for evaluator loop checkpoints.
    /// Unlimited and unarmed budgets install a no-op owner scope, so another
    /// driver cannot replace control while this evaluator is in use.
    ///
    /// # Errors
    /// Another driver owns the evaluator's control registration.
    pub fn enter(
        self: &Arc<Self>,
        evaluator: &dive_eval::Evaluator,
    ) -> Result<dive_eval::HostControlScope, dive_eval::HostControlConflict> {
        evaluator.host_control(Some(self.clone()))
    }

    /// Matches this context's interruption to actual exhaustion or measurement failure.
    /// A genuine evaluation error or another owner's interruption returns `None`.
    #[must_use]
    pub fn verdict(&self, error: &dive_eval::EvalError) -> Option<crate::Verdict> {
        if !self.owns(error) {
            return None;
        }
        match self.budget.poll() {
            Ok(Some(exhausted)) => Some(crate::Verdict::Stopped(Box::new(
                exhausted.during(self.phase),
            ))),
            Err(error) => Some(crate::Verdict::BudgetMeasurement { error }),
            Ok(None) => None,
        }
    }

    /// Whether this typed interruption belongs to the same armed run.
    /// Identity alone is not proof that a resource was exhausted.
    #[must_use]
    pub fn owns(&self, error: &dive_eval::EvalError) -> bool {
        self.identity != 0
            && self.budget.is_armed()
            && !self.budget.is_unlimited()
            && error
                .interruption()
                .is_some_and(|interruption| interruption.token() == self.identity)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "deterministic armed-control evidence")]
mod evaluator_controls {
    use super::*;
    use dive_eval::HostControl;

    #[test]
    fn measurement_failure_interrupts_only_its_owner_and_never_fabricates_exhaustion() {
        let watchdog = Budget::new(None, Some(100))
            .start_with_sampler(
                |_, _, _| std::thread::Builder::new().spawn(|| {}),
                Sampler::Injected(Arc::new(|| Some(10))),
            )
            .expect("initial measurement")
            .expect("finite budget");
        let control = EvaluationControl::new(watchdog.budget(), Phase::Search).expect("control");
        assert_eq!(control.check(), Ok(()));
        let failure = MeasurementError::unavailable(SamplePoint::Periodic);
        assert_eq!(
            watchdog
                .budget()
                .signal
                .as_ref()
                .expect("armed")
                .record_sample(SamplePoint::Periodic, None, 100),
            Err(failure)
        );
        let error = dive_eval::EvalError::interrupted(control.check().expect_err("interrupt"));
        assert_eq!(
            control.verdict(&error),
            Some(crate::Verdict::BudgetMeasurement { error: failure })
        );
        let temporal =
            EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("same run");
        assert_eq!(
            temporal.verdict(&error),
            Some(crate::Verdict::BudgetMeasurement { error: failure })
        );
        let genuine = dive_eval::EvalError::general("host interrupted evaluation");
        assert!(control.verdict(&genuine).is_none());
        let other = Budget::new(Some(Duration::from_secs(3600)), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        let other = EvaluationControl::new(other.budget(), Phase::Search).expect("other owner");
        assert!(other.verdict(&error).is_none());
    }

    #[test]
    fn contexts_require_actual_exhaustion_of_the_same_signal_and_keep_the_current_phase() {
        let watchdog = Budget::new(Some(Duration::from_secs(3600)), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        let budget = watchdog.budget();
        let search = EvaluationControl::new(budget, Phase::Search).expect("search control");
        let temporal = EvaluationControl::new(budget, Phase::Temporal).expect("temporal control");
        let fake = dive_eval::EvalError::interrupted(dive_eval::Interrupted::new(search.identity));
        assert!(search.owns(&fake));
        assert!(
            search.verdict(&fake).is_none(),
            "identity alone is not exhaustion"
        );
        budget.trip_for_test();
        let error = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    dive_eval::EvalError::interrupted(search.check().expect_err("interrupted"))
                })
                .join()
                .expect("worker")
        });
        assert!(
            matches!(search.verdict(&error),Some(crate::Verdict::Stopped(stop)) if stop.phase==Phase::Search)
        );
        assert!(
            matches!(temporal.verdict(&error),Some(crate::Verdict::Stopped(stop)) if stop.phase==Phase::Temporal)
        );
        let other_watchdog = Budget::new(Some(Duration::from_secs(3600)), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        other_watchdog.budget().trip_for_test();
        let other =
            EvaluationControl::new(other_watchdog.budget(), Phase::Capture).expect("other run");
        assert!(!other.owns(&error));
        assert!(other.verdict(&error).is_none());
        for unarmed in [Budget::unlimited(), Budget::new(Some(Duration::ZERO), None)] {
            let control = EvaluationControl::new(&unarmed, Phase::Search).expect("control");
            assert!(!control.owns(&error));
            assert!(control.verdict(&error).is_none());
        }
        let genuine = dive_eval::EvalError::general("host interrupted evaluation");
        assert!(search.verdict(&genuine).is_none());
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "deterministic assumption control evidence"
)]
mod assumption_controls {
    use super::*;

    const LOOP: &str = "PrintT(\"before-loop\") /\\ (\\A i \\in 1..4096: i = i)";

    fn model(assumption: &str, options: &crate::CheckOptions) -> dive_model::Model {
        let source = format!(
            "---- MODULE ControlledAssume ----\nEXTENDS Naturals, TLC\nVARIABLE x\n\
             ASSUME {assumption}\nInit == x = 0\nNext == x' = x\n====\n"
        );
        let analysis = dive_sema::analyze_source(
            "ControlledAssume.tla",
            &source,
            &dive_sema::Options::default(),
        );
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let config = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("config");
        dive_model::bind(
            ir,
            &config,
            &dive_model::BindOptions {
                eval: options.eval,
                symmetry_liveness: options.symmetry_liveness,
                no_deadlock: false,
                host: None,
            },
        )
        .expect("bind")
    }

    #[test]
    fn producer_assumption_loops_stop_with_their_owner_and_restore_control() {
        for options in [crate::CheckOptions::tlc(0, 0), crate::CheckOptions::dive(0)] {
            let model = model(LOOP, &options);
            let watchdog = Budget::new(Some(Duration::from_secs(3600)), None)
                .start()
                .expect("watchdog")
                .expect("finite budget");
            watchdog.budget().trip_for_test();
            assert!(matches!(
                check_assumptions(&model, watchdog.budget()),
                Some(crate::Verdict::Stopped(stop)) if stop.phase == Phase::Startup
            ));
            let prints = model.take_output();
            assert_eq!(prints.len(), 1);
            assert!(prints[0].contains("before-loop"));
            // A stopped assumption did not cache false/error truth, and its
            // scope did not leave the expired capability on this evaluator.
            assert!(matches!(
                model.check_assumptions(),
                dive_eval::AssumptionOutcome::AllTrue
            ));
            assert_eq!(model.take_output().len(), 1);
        }
    }

    #[test]
    fn real_assumption_failures_keep_their_class_before_a_checkpoint() {
        for options in [crate::CheckOptions::tlc(0, 0), crate::CheckOptions::dive(0)] {
            let watchdog = Budget::new(Some(Duration::from_secs(3600)), None)
                .start()
                .expect("watchdog")
                .expect("finite budget");
            watchdog.budget().trip_for_test();
            // A deciding false/error before a cooperative checkpoint keeps
            // its genuine class, including when the signal has already tripped.
            for budget in [&Budget::unlimited(), watchdog.budget()] {
                assert!(matches!(
                    check_assumptions(&model("FALSE", &options), budget),
                    Some(crate::Verdict::AssumptionFalse { .. })
                ));
                assert!(matches!(
                    check_assumptions(&model("1 \\div 0 = 0", &options), budget),
                    Some(crate::Verdict::EvaluationError {
                        site: crate::ErrorSite::Assumption,
                        code: 2105,
                        reported: 2105,
                        ..
                    })
                ));
                assert_eq!(check_assumptions(&model("TRUE", &options), budget), None);
            }
        }
    }

    #[test]
    fn assumption_measurement_failure_remains_a_measurement_failure() {
        for options in [crate::CheckOptions::tlc(0, 0), crate::CheckOptions::dive(0)] {
            let model = model(LOOP, &options);
            let watchdog = Budget::new(None, Some(100))
                .start_with_sampler(
                    |_, _, _| std::thread::Builder::new().spawn(|| {}),
                    Sampler::Injected(Arc::new(|| Some(10))),
                )
                .expect("initial measurement")
                .expect("finite budget");
            let failure = MeasurementError::unavailable(SamplePoint::Periodic);
            assert_eq!(
                watchdog
                    .budget()
                    .signal
                    .as_ref()
                    .expect("armed")
                    .record_sample(SamplePoint::Periodic, None, 100),
                Err(failure)
            );
            assert_eq!(
                check_assumptions(&model, watchdog.budget()),
                Some(crate::Verdict::BudgetMeasurement { error: failure })
            );
            assert_eq!(model.take_output().len(), 1);
        }
    }
}
