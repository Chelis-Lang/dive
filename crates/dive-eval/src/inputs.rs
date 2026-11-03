//! Explicit host categories and bounded, source-ordered external input replay.
//!
//! Recorded values are assumptions supplied by the caller. Replay proves their
//! use by source operations; it cannot prove what a past operating system held.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use dive_ir::{StdOp, StdSupport};

use crate::HostError;

/// Maximum total UTF-8 bytes retained by one external-input context.
pub const MAX_INPUT_BYTES: usize = 1 << 20;
/// Maximum source input operations retained by one context.
pub const MAX_INPUT_RECORDS: usize = 256;
/// Maximum entries in one captured environment.
pub const MAX_ENV_ENTRIES: usize = 4096;

/// Explicit host category grants (D-017). Every profile defaults to no grants.
/// A grant never supplies an implementation or overrides a driver's refusal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HostAccess {
    /// Read the process environment.
    pub env: bool,
    /// Read or write files, where the chosen driver implements the operation.
    pub files: bool,
    /// Execute processes, where implemented. Currently refused by the drivers.
    pub exec: bool,
}

impl HostAccess {
    /// No host access, including under TLC compatibility.
    pub const NONE: Self = Self {
        env: false,
        files: false,
        exec: false,
    };

    /// Parses the approved comma-separated category spelling.
    ///
    /// ```
    /// let access = dive_eval::inputs::HostAccess::parse("env,files")?;
    /// assert!(access.env && access.files && !access.exec);
    /// # Ok::<(), &'static str>(())
    /// ```
    ///
    /// # Errors
    /// Empty, duplicate or unknown category names.
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let mut access = Self::NONE;
        for name in text.split(',') {
            let slot = match name {
                "env" => &mut access.env,
                "files" => &mut access.files,
                "exec" => &mut access.exec,
                _ => return Err("--allow-io expects comma-separated env,files,exec categories"),
            };
            if *slot {
                return Err("--allow-io repeats a category");
            }
            *slot = true;
        }
        Ok(access)
    }

    /// Granted category names in stable report order.
    #[must_use]
    pub fn names(self) -> Vec<&'static str> {
        [
            (self.env, "env"),
            (self.files, "files"),
            (self.exec, "exec"),
        ]
        .into_iter()
        .filter_map(|(yes, name)| yes.then_some(name))
        .collect()
    }

    /// Whether no category was granted.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.env && !self.files && !self.exec
    }

    /// Whether a native operator's host category is granted. Non-I/O operators
    /// require no category; this says nothing about implementation support.
    #[must_use]
    pub fn allows(self, op: StdOp) -> bool {
        self.denied(op).is_none()
    }

    pub(crate) fn denied(self, op: StdOp) -> Option<&'static str> {
        let (allowed, category) = match op {
            StdOp::IoUtilsIOEnv => (self.env, "env"),
            StdOp::IoUtilsIOExec
            | StdOp::IoUtilsIOEnvExec
            | StdOp::IoUtilsIOExecTemplate
            | StdOp::IoUtilsIOEnvExecTemplate => (self.exec, "exec"),
            _ if op.support() == StdSupport::Io => (self.files, "files"),
            _ => return None,
        };
        (!allowed).then_some(category)
    }
}

/// A source operation's external request, after its real arguments evaluate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputRequest {
    /// `IOUtils!IOEnv`.
    Environment,
    /// `IOUtils!Deserialize` with admitted TXT/UTF-8 options.
    TextFile(String),
}

/// External bytes interpreted by the evaluator, never an arbitrary TLA+ value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputResponse {
    /// The complete UTF-8 process environment, fixed for a run.
    Environment(BTreeMap<String, String>),
    /// Exact UTF-8 contents of one file read.
    Text(String),
}

/// One actual source-ordered input operation and its assumed response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputRecord {
    /// The source operation and actual path argument.
    pub request: InputRequest,
    /// The recorded external input, not a producer state or proof flag.
    pub response: InputResponse,
}

impl InputRecord {
    /// Retained UTF-8 bytes, including names and path arguments.
    #[must_use]
    pub fn bytes(&self) -> usize {
        let path = match &self.request {
            InputRequest::Environment => 0,
            InputRequest::TextFile(path) => path.len(),
        };
        path.saturating_add(match &self.response {
            InputResponse::Environment(entries) => {
                entries.iter().fold(0usize, |n, (key, value)| {
                    n.saturating_add(key.len()).saturating_add(value.len())
                })
            }
            InputResponse::Text(text) => text.len(),
        })
    }
}

/// Bounded external assumptions for a fresh source binding.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordedInputs {
    /// Explicit categories used by the original request.
    pub grants: HostAccess,
    /// Operations in their actual source execution order.
    pub records: Vec<InputRecord>,
}

/// Raw external input provider owned by one source-binding operation.
pub trait InputSource: Send + Sync + std::fmt::Debug {
    /// Executes or replays one exact source request.
    ///
    /// # Errors
    /// A named unsupported request, resource bound or replay mismatch.
    fn request(&self, request: &InputRequest) -> Result<InputResponse, HostError>;
    /// Closes binding and checks all required source demand.
    ///
    /// # Errors
    /// A remembered error or undemanded extra input records.
    fn finish_binding(&self) -> Result<(), HostError>;
    /// Captured assumptions as raw text, never evaluator-owned values.
    fn recorded(&self) -> RecordedInputs;
}

impl RecordedInputs {
    /// Checks resource bounds and response/request consistency without I/O.
    ///
    /// # Errors
    /// Oversized, incorrectly typed, ungranted or inconsistent input records.
    pub fn validate(&self) -> Result<(), HostError> {
        if self.records.len() > MAX_INPUT_RECORDS
            || self
                .records
                .iter()
                .fold(0usize, |n, r| n.saturating_add(r.bytes()))
                > MAX_INPUT_BYTES
        {
            return Err(input_error("recorded external input bounds"));
        }
        let mut environment = None;
        for record in &self.records {
            match (&record.request, &record.response) {
                (InputRequest::Environment, InputResponse::Environment(entries))
                    if self.grants.env =>
                {
                    if entries.len() > MAX_ENV_ENTRIES {
                        return Err(input_error("environment entry limit"));
                    }
                    if environment.is_some_and(|previous| previous != entries) {
                        return Err(input_error("recorded IOEnv must retain one environment"));
                    }
                    environment = Some(entries);
                }
                (InputRequest::TextFile(_), InputResponse::Text(_)) if self.grants.files => {}
                _ => return Err(input_error("recorded input kind or category mismatch")),
            }
        }
        Ok(())
    }
}

/// A fresh ordered replay cursor. It has no operating-system access.
#[derive(Debug)]
pub struct InputReplay {
    inputs: RecordedInputs,
    cursor: Mutex<(usize, bool, Option<HostError>)>,
}

impl InputReplay {
    /// Starts an independent cursor over explicit, bounded assumptions.
    ///
    /// ```
    /// use dive_eval::inputs::{HostAccess, InputRecord, InputReplay,
    ///     InputRequest, InputResponse, RecordedInputs};
    /// let request = InputRequest::TextFile("input.txt".into());
    /// let replay = InputReplay::new(RecordedInputs {
    ///     grants: HostAccess::parse("files").unwrap(),
    ///     records: vec![InputRecord { request: request.clone(),
    ///         response: InputResponse::Text("hello".into()) }],
    /// }).unwrap();
    /// assert_eq!(replay.request(&request).unwrap(), InputResponse::Text("hello".into()));
    /// replay.finish_binding().unwrap();
    /// ```
    ///
    /// # Errors
    /// Invalid recorded inputs; no source or host callback is executed.
    pub fn new(inputs: RecordedInputs) -> Result<Self, HostError> {
        inputs.validate()?;
        Ok(Self {
            inputs,
            cursor: Mutex::new((0, false, None)),
        })
    }

    /// Consumes the next exact source operation/argument pair.
    ///
    /// # Errors
    /// Missing, mismatched, extra-phase or previously failed input requests.
    pub fn request(&self, request: &InputRequest) -> Result<InputResponse, HostError> {
        let mut cursor = self.cursor.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = &cursor.2 {
            return Err(error.clone());
        }
        let record = self.inputs.records.get(cursor.0);
        let result = if cursor.1 {
            Err(input_error("external input outside source binding"))
        } else if let Some(record) = record {
            if &record.request == request {
                Ok(record.response.clone())
            } else {
                Err(input_error(
                    "recorded input does not match the next source operation and arguments",
                ))
            }
        } else {
            Err(input_error("missing recorded source input"))
        };
        match &result {
            Ok(_) => cursor.0 += 1,
            Err(error) => cursor.2 = Some(error.clone()),
        }
        result
    }

    /// Closes binding, requiring every supplied record to have been demanded.
    ///
    /// # Errors
    /// A latched replay failure or extra input records.
    pub fn finish_binding(&self) -> Result<(), HostError> {
        let mut cursor = self.cursor.lock().unwrap_or_else(PoisonError::into_inner);
        cursor.1 = true;
        if let Some(error) = &cursor.2 {
            return Err(error.clone());
        }
        if cursor.0 != self.inputs.records.len() {
            let error = input_error("extra recorded inputs not demanded by source binding");
            cursor.2 = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// A latched mismatch or request outside binding, even if source caught it.
    #[must_use]
    pub fn failure(&self) -> Option<HostError> {
        self.cursor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .2
            .clone()
    }
}

impl InputSource for InputReplay {
    fn request(&self, request: &InputRequest) -> Result<InputResponse, HostError> {
        Self::request(self, request)
    }
    fn finish_binding(&self) -> Result<(), HostError> {
        Self::finish_binding(self)
    }
    fn recorded(&self) -> RecordedInputs {
        self.inputs.clone()
    }
}

impl crate::Host for InputReplay {
    fn input(&self, request: &InputRequest) -> Option<Result<InputResponse, HostError>> {
        Some(self.request(request))
    }
}

/// A named external-input refusal; code zero never certifies a TLC exception.
#[must_use]
pub fn input_error(message: &str) -> HostError {
    HostError {
        code: 0,
        message: format!("external inputs: {message}"),
    }
}
