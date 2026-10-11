//! TLC's periodic-work control flow over explicit backend callbacks.
//!
//! This kernel preserves queue suspension, force clearing, PERIODIC and
//! checkpoint ordering. It does not certify graph construction, SCC selection,
//! clock sampling, checkpoint persistence or an ordinary model-checking verdict.
//! The backend must supply those operations and their independent evidence.

/// Current policy and the already-consumed checkpoint request for one invocation.
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent pinned controller inputs; all selected combinations have primary controls"
)]
pub struct Policy {
    /// Whether the configured model has liveness work.
    pub check_liveness: bool,
    /// TLC's current measured fraction of runtime spent checking liveness.
    pub runtime_ratio: f64,
    /// TLC's configured upper bound on that fraction (normally 0.2).
    pub ratio_limit: f64,
    /// Outstanding forced liveness request; cleared only after a successful check.
    pub force_liveness: bool,
    /// Checkpoint request consumed once before entering this invocation.
    pub checkpoint: bool,
    /// Whether an actual configured `_PERIODIC` expression is present.
    pub periodic: bool,
}

/// Deciding result of one periodic invocation; none is a model-proof result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Work completed or queue suspension did not succeed.
    Complete,
    /// A liveness callback returned this exact nonzero TLC message code.
    LivenessStopped(i32),
    /// The actual `_PERIODIC` expression evaluated to FALSE (TLC code 2104).
    PeriodicAssumptionFalse,
}

/// A backend callback failure or an invalid elapsed wall-clock observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error<E> {
    /// Original backend failure, retained without converting it into success.
    Callback(E),
    /// The elapsed liveness time was negative after Java-compatible subtraction.
    ClockWentBackwards,
}

/// Invalid elapsed time supplied to the runtime-ratio arithmetic primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatioError {
    /// Negative elapsed liveness time; the pinned updater asserts this condition.
    NegativeElapsed,
}

/// Calculate TLC's runtime fraction from explicit millisecond observations.
///
/// Preserve Java signed-long wrapping before conversion to double, its positive
/// zero clamp and NaN propagation. This arithmetic primitive does not sample a
/// clock, certify its origin or establish an ordinary checking schedule.
/// `progress_interval` is the signed Java integer interval in milliseconds.
///
/// # Errors
/// Negative elapsed liveness time is refused explicitly.
#[allow(
    clippy::cast_precision_loss,
    reason = "the pinned updater explicitly converts Java long arithmetic to double"
)]
pub fn calculate_runtime_ratio(
    previous: f64,
    start_time: i64,
    now: i64,
    progress_interval: i32,
    elapsed: i64,
) -> Result<f64, RatioError> {
    if elapsed < 0 {
        return Err(RatioError::NegativeElapsed);
    }
    let interval = i64::from(progress_interval);
    let total = now
        .wrapping_sub(start_time)
        .wrapping_sub(interval)
        .wrapping_sub(elapsed);
    let product = total as f64 * previous;
    // f64::max suppresses NaN; Java Math.max must preserve it and clamp -0 to +0.
    let absolute = if product <= 0.0 { 0.0 } else { product };
    let denominator = total.wrapping_add(interval).wrapping_add(elapsed);
    Ok((elapsed as f64 + absolute) / denominator as f64)
}

/// Backend operations in the order dictated by the pinned TLC controller.
///
/// Implementations must retain concrete source/graph authority. In particular,
/// supplied callback outcomes do not establish graph or model conformance.
pub trait Hooks {
    /// The backend's typed failure, including any source/bound information.
    type Error;
    /// Test graph growth; the controller preserves TLC's short-circuit calls.
    ///
    /// # Errors
    /// The backend cannot obtain source-authorized graph growth.
    fn growth_due(&mut self) -> Result<bool, Self::Error>;
    /// Suspend the actual worker queue, returning TLC's success condition.
    ///
    /// # Errors
    /// The backend fails to suspend the actual worker queue.
    fn suspend_workers(&mut self) -> Result<bool, Self::Error>;
    /// Run the actual partial liveness check and return its TLC message code.
    ///
    /// # Errors
    /// The backend fails before obtaining a deciding SCC message code.
    fn check_liveness(&mut self, forced: bool) -> Result<i32, Self::Error>;
    /// Sample the backend's wall clock in milliseconds.
    ///
    /// # Errors
    /// The backend cannot obtain its wall-clock sample.
    fn time_millis(&mut self) -> Result<i64, Self::Error>;
    /// Update the runtime fraction using this observed elapsed liveness time.
    ///
    /// # Errors
    /// The backend cannot update its actual runtime measurement.
    fn update_runtime_ratio(&mut self, elapsed_millis: i64) -> Result<f64, Self::Error>;
    /// Evaluate TLC's FALSE-equality test on the actual `_PERIODIC` value.
    /// Preserve non-boolean comparison failures through the callback error.
    ///
    /// # Errors
    /// Evaluation or TLC FALSE-equality comparison fails.
    fn periodic_is_false(&mut self) -> Result<bool, Self::Error>;
    /// Execute the checkpoint transaction, including TLC's internal queue resume.
    ///
    /// # Errors
    /// The checkpoint transaction fails; preserve its partial evidence.
    fn checkpoint(&mut self) -> Result<(), Self::Error>;
    /// Resume the actual queue when no checkpoint transaction is requested.
    ///
    /// # Errors
    /// The backend cannot resume its suspended worker queue.
    fn resume_workers(&mut self) -> Result<(), Self::Error>;
}

/// Execute one pinned TLC periodic-work invocation.
///
/// The caller consumes the global checkpoint request before calling this function.
/// A nonzero liveness code or FALSE PERIODIC stops before checkpointing or resuming;
/// at the exact ratio boundary an unforced check is not performed. Force requests
/// survive failed suspension and nonzero liveness results, as in TLC.
///
/// # Errors
/// Any backend callback fails, or its elapsed liveness clock moves backwards.
pub fn run<H: Hooks>(policy: &mut Policy, hooks: &mut H) -> Result<Outcome, Error<H::Error>> {
    let no_liveness_work = !policy.check_liveness
        || policy.runtime_ratio > policy.ratio_limit
        || !hooks.growth_due().map_err(Error::Callback)?;
    if no_liveness_work && !policy.force_liveness && !policy.checkpoint && !policy.periodic {
        policy.runtime_ratio = hooks.update_runtime_ratio(0).map_err(Error::Callback)?;
        return Ok(Outcome::Complete);
    }
    if !hooks.suspend_workers().map_err(Error::Callback)? {
        return Ok(Outcome::Complete);
    }
    if policy.check_liveness && (policy.runtime_ratio < policy.ratio_limit || policy.force_liveness)
    {
        let before = hooks.time_millis().map_err(Error::Callback)?;
        let code = hooks
            .check_liveness(policy.force_liveness)
            .map_err(Error::Callback)?;
        if code != 0 {
            return Ok(Outcome::LivenessStopped(code));
        }
        policy.force_liveness = false;
        let elapsed = hooks
            .time_millis()
            .map_err(Error::Callback)?
            .wrapping_sub(before);
        if elapsed < 0 {
            return Err(Error::ClockWentBackwards);
        }
        policy.runtime_ratio = hooks
            .update_runtime_ratio(elapsed)
            .map_err(Error::Callback)?;
    } else if policy.runtime_ratio > policy.ratio_limit {
        policy.runtime_ratio = hooks.update_runtime_ratio(0).map_err(Error::Callback)?;
    }
    if policy.periodic && hooks.periodic_is_false().map_err(Error::Callback)? {
        return Ok(Outcome::PeriodicAssumptionFalse);
    }
    if policy.checkpoint {
        hooks.checkpoint().map_err(Error::Callback)?;
    } else {
        hooks.resume_workers().map_err(Error::Callback)?;
    }
    Ok(Outcome::Complete)
}
