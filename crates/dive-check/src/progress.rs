//! Producer-clock requests coordinated with actual expansion claims.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Native diagnostic request policy. No caller clock or request hook is used.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Producer timer interval. Zero refuses instead of spinning.
    pub interval: Duration,
    /// One initial request, published before expansion claims.
    pub force: bool,
}
impl Options {
    /// Validate before producer clock/thread setup.
    ///
    /// # Errors
    /// Intervals outside 1 through 2,147,483,647 milliseconds.
    pub fn validate(self) -> Result<(), String> {
        if self.interval < Duration::from_millis(1)
            || self.interval > Duration::from_millis(i32::MAX as u64)
        {
            Err("native request interval outside 1..=2147483647 milliseconds".into())
        } else {
            Ok(())
        }
    }
}
/// Internally generated request; neither callers nor observers can construct it.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    sequence: u64,
    millis: u128,
    forced: bool,
}
impl Request {
    /// Producer-owned monotonically increasing request identity.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Actual producer UNIX-millisecond sample, not a supplied timestamp.
    #[must_use]
    pub fn millis(&self) -> u128 {
        self.millis
    }
    /// Initial forced request rather than a producer timer expiration.
    #[must_use]
    pub fn forced(&self) -> bool {
        self.forced
    }
}
struct State {
    pending: Option<Request>,
    sequence: u64,
    closed: bool,
    cancelled: bool,
    error: Option<String>,
}
pub(crate) struct Lane {
    state: Mutex<State>,
    wake: Condvar,
    interval: Duration,
}
impl Lane {
    pub(crate) fn new(options: Options) -> Result<Arc<Self>, String> {
        options.validate()?;
        let lane = Arc::new(Self {
            state: Mutex::new(State {
                pending: None,
                sequence: 0,
                closed: false,
                cancelled: false,
                error: None,
            }),
            wake: Condvar::new(),
            interval: options.interval,
        });
        if options.force {
            lane.publish(true);
        }
        Ok(lane)
    }
    fn publish(&self, forced: bool) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed || state.cancelled || state.pending.is_some() || state.error.is_some() {
            return;
        }
        let Some(sequence) = state.sequence.checked_add(1) else {
            state.error = Some("native request identity exhausted".into());
            return;
        };
        let Ok(time) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            state.error = Some("native request clock precedes UNIX epoch".into());
            return;
        };
        state.sequence = sequence;
        state.pending = Some(Request {
            sequence,
            millis: time.as_millis(),
            forced,
        });
        self.wake.notify_all();
    }
    pub(crate) fn timer(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            while state.pending.is_some() && !state.closed && !state.cancelled {
                state = self
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if state.closed || state.cancelled || state.error.is_some() {
                return;
            }
            let (next, timeout) = self
                .wake
                .wait_timeout(state, self.interval)
                .unwrap_or_else(PoisonError::into_inner);
            state = next;
            if timeout.timed_out() && !state.closed {
                drop(state);
                self.publish(false);
                state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            }
        }
    }
    /// The same lock serializes timer publication and admission of new claims.
    pub(crate) fn claim(&self, next: &AtomicUsize, count: usize) -> Option<usize> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.pending.is_some() || state.closed || (state.error.is_some() && !state.cancelled) {
            return None;
        }
        let claim = next.fetch_add(1, Ordering::Relaxed);
        (claim < count).then_some(claim)
    }
    pub(crate) fn take(&self) -> Result<Option<Request>, String> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        let request = state.pending.take();
        self.wake.notify_all();
        Ok(request)
    }
    /// Disable new requests after an actual expansion cut without closing claim
    /// admission. Earlier unclaimed sources must still find the ordered winner.
    pub(crate) fn cancel_requests(&self) -> (bool, Option<Request>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let fresh = !state.cancelled;
        state.cancelled = true;
        let pending = state.pending.take();
        self.wake.notify_all();
        (fresh, pending)
    }
    pub(crate) fn close(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .closed = true;
        self.wake.notify_all();
    }
}

struct Close(Arc<Lane>);
impl Drop for Close {
    fn drop(&mut self) {
        self.0.close();
    }
}
pub(crate) fn with_lane<R>(
    options: Option<Options>,
    callback: impl FnOnce(Option<Arc<Lane>>) -> R,
) -> Result<R, String> {
    let Some(options) = options else {
        return Ok(callback(None));
    };
    let lane = Lane::new(options)?;
    std::thread::scope(|scope| {
        let timer = lane.clone();
        let handle = std::thread::Builder::new()
            .name("dive-progress-request".into())
            .spawn_scoped(scope, move || timer.timer())
            .map_err(|e| e.to_string())?;
        let close = Close(lane.clone());
        let result = callback(Some(lane));
        drop(close);
        handle
            .join()
            .map_err(|_| "native request producer panicked".to_owned())?;
        Ok(result)
    })
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "bounded producer clock and claim controls"
)]
mod tests {
    use super::*;
    #[test]
    fn actual_timer_can_request_while_a_claim_is_still_in_flight() {
        with_lane(
            Some(Options {
                interval: Duration::from_millis(1),
                force: false,
            }),
            |lane| {
                let lane = lane.expect("actual producer");
                let claims = AtomicUsize::new(0);
                // This represents an admitted claim which has not yet returned.
                assert_eq!(lane.claim(&claims, 8), Some(0));
                let state = lane.state.lock().expect("state");
                let (state, timeout) = lane
                    .wake
                    .wait_timeout_while(state, Duration::from_secs(5), |state| {
                        state.pending.is_none()
                    })
                    .expect("timer wait");
                assert!(!timeout.timed_out());
                assert!(!state.pending.expect("real timed request").forced());
                drop(state);
                assert_eq!(lane.claim(&claims, 8), None);
                assert_eq!(claims.load(Ordering::Relaxed), 1);
                let request = lane.take().expect("service").expect("pending request");
                assert_eq!(request.sequence(), 1);
                assert_eq!(lane.claim(&claims, 8), Some(1));
            },
        )
        .expect("closed producer");
    }
    #[test]
    fn forced_request_blocks_claims_until_service_and_bad_intervals_refuse() {
        let lane = Lane::new(Options {
            interval: Duration::from_secs(1),
            force: true,
        })
        .expect("lane");
        let claims = AtomicUsize::new(0);
        assert_eq!(lane.claim(&claims, 8), None);
        assert!(lane.take().expect("service").expect("forced").forced());
        assert_eq!(lane.claim(&claims, 8), Some(0));
        lane.publish(false);
        assert_eq!(lane.claim(&claims, 8), None);
        assert_eq!(lane.take().expect("service").expect("second").sequence(), 2);
        lane.close();
        assert_eq!(lane.claim(&claims, 8), None);
        for interval in [Duration::ZERO, Duration::from_nanos(1), Duration::MAX] {
            assert!(
                Options {
                    interval,
                    force: true
                }
                .validate()
                .is_err()
            );
        }
    }
    #[test]
    fn cancellation_after_later_error_keeps_earlier_claims_open() {
        with_lane(
            Some(Options {
                interval: Duration::from_millis(1),
                force: true,
            }),
            |lane| {
                let lane = lane.expect("producer");
                let claims = AtomicUsize::new(0);
                // A chaos-ordered later source has errored while logical source zero
                // is still unclaimed. Cancellation must drain the real earlier work.
                let (fresh, pending) = lane.cancel_requests();
                assert!(fresh);
                assert!(pending.expect("pending request").forced());
                for expected in 0..8 {
                    lane.publish(false);
                    assert_eq!(lane.claim(&claims, 8), Some(expected));
                    assert!(lane.take().expect("cancelled timer").is_none());
                }
                assert!(!lane.cancel_requests().0);
                assert_eq!(lane.claim(&claims, 8), None);
                assert!(!lane.state.lock().expect("state").closed);
                lane.close();
                assert_eq!(lane.claim(&AtomicUsize::new(0), 8), None);
            },
        )
        .expect("joined cancelled timer");
    }
    #[test]
    #[allow(clippy::panic, reason = "controlled producer close on callback unwind")]
    fn callback_error_and_unwind_close_and_join_the_actual_timer() {
        for unwind in [false, true] {
            let mut weak = None;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_lane(
                    Some(Options {
                        interval: Duration::from_millis(1),
                        force: true,
                    }),
                    |lane| {
                        let lane = lane.expect("lane");
                        weak = Some(Arc::downgrade(&lane));
                        assert!(!unwind, "controlled request callback unwind");
                        Err::<(), _>("controlled request callback error")
                    },
                )
            }));
            assert_eq!(result.is_err(), unwind);
            if !unwind {
                assert_eq!(
                    result.expect("returned error"),
                    Ok(Err("controlled request callback error"))
                );
            }
            assert!(weak.expect("actual timer owner").upgrade().is_none());
        }
    }
}
