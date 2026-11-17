//! Per-thread capture of `Print` output, for a parallel checker that
//! releases output in TLC's sequential order.
//!
//! While a [`PrintCapture`] is active on a thread, `Print`, `PrintT` and the
//! evaluator's once-per-run warnings evaluated on that thread go into the
//! capture instead of the evaluator's sink. The checker drains the capture
//! at the boundaries of the reference order (before a successor, after its
//! constraints, after its invariants, ...), tags each batch with its
//! position, and later releases the batches in position order through
//! [`crate::Evaluator::release`].
//!
//! Warnings are printed once per run. Which evaluation prints a warning
//! must not depend on scheduling, so a captured warning is not checked
//! against the run's warning history when it is raised: [`Captured::warning`]
//! marks it, and [`crate::Evaluator::release`] keeps it only if it is the
//! first of its text released in the run.
//!
//! ```
//! use dive_eval::capture::PrintCapture;
//!
//! let cap = PrintCapture::start();
//! assert!(cap.take().is_empty());
//! drop(cap); // output goes to the evaluator's sink again
//! ```

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

/// One captured line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Captured {
    /// The text, as the sink would receive it.
    pub text: String,
    /// Whether it is a once-per-run warning (see the module documentation).
    pub warning: bool,
}

thread_local! {
    static CAPTURE: RefCell<Option<Vec<Captured>>> = const { RefCell::new(None) };
}

/// Captures `Print` output on the current thread while it lives. Captures
/// nest: dropping one restores the enclosing capture, whose lines are kept.
#[derive(Debug)]
pub struct PrintCapture {
    saved: Option<Vec<Captured>>,
    /// Bound to the thread it started on.
    _thread: PhantomData<Rc<()>>,
}

impl PrintCapture {
    /// Starts capturing on the current thread.
    #[must_use]
    pub fn start() -> Self {
        let saved = CAPTURE.with(|c| c.borrow_mut().replace(Vec::new()));
        Self {
            saved,
            _thread: PhantomData,
        }
    }

    /// Takes the lines captured since the last call.
    #[must_use]
    pub fn take(&self) -> Vec<Captured> {
        CAPTURE.with(|c| {
            c.borrow_mut()
                .as_mut()
                .map(std::mem::take)
                .unwrap_or_default()
        })
    }
}

impl Drop for PrintCapture {
    fn drop(&mut self) {
        let saved = self.saved.take();
        CAPTURE.with(|c| *c.borrow_mut() = saved);
    }
}

/// Adds a line to the current thread's capture. Returns the line back if no
/// capture is active.
pub(crate) fn push(text: String, warning: bool) -> Option<String> {
    CAPTURE.with(|c| match c.borrow_mut().as_mut() {
        Some(lines) => {
            lines.push(Captured { text, warning });
            None
        }
        None => Some(text),
    })
}

/// Whether a capture is active on the current thread.
pub(crate) fn active() -> bool {
    CAPTURE.with(|c| c.borrow().is_some())
}
