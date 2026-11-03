//! Bounded live environment/text inputs during source binding (D-017).
//! Later evaluation can use captured values, but cannot execute more I/O.

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Mutex, PoisonError};

use dive_eval::HostError;
use dive_eval::inputs::{
    HostAccess, InputRecord, InputRequest, InputResponse, InputSource, MAX_ENV_ENTRIES,
    MAX_INPUT_BYTES, MAX_INPUT_RECORDS, RecordedInputs, input_error,
};

#[derive(Debug)]
struct State {
    records: Vec<InputRecord>,
    bytes: usize,
    environment: Option<BTreeMap<String, String>>,
    closed: bool,
    failure: Option<HostError>,
}

/// Live source-binding inputs, empty and unopened until source requests them.
/// Granting `env` records the entire UTF-8 environment; it may contain secrets.
/// A seed alone cannot reproduce these external assumptions.
#[derive(Debug)]
pub struct LiveInputs {
    grants: HostAccess,
    state: Mutex<State>,
}

impl LiveInputs {
    /// Creates a bounded recording context. This performs no host access.
    #[must_use]
    pub fn new(grants: HostAccess) -> Self {
        Self {
            grants,
            state: Mutex::new(State {
                records: Vec::new(),
                bytes: 0,
                environment: None,
                closed: false,
                failure: None,
            }),
        }
    }

    fn read(&self, state: &mut State, request: &InputRequest) -> Result<InputResponse, HostError> {
        if state.closed {
            return Err(input_error(
                "runtime host reads are not implemented; capture inputs in source definitions",
            ));
        }
        if state.records.len() >= MAX_INPUT_RECORDS {
            return Err(input_error("source input record limit"));
        }
        let response = match request {
            InputRequest::Environment if self.grants.env => {
                if state.environment.is_none() {
                    let mut environment = BTreeMap::new();
                    let mut bytes = 0usize;
                    for (name, value) in std::env::vars_os() {
                        if environment.len() >= MAX_ENV_ENTRIES {
                            return Err(input_error("environment entry limit"));
                        }
                        let (name, value) = (
                            name.into_string()
                                .map_err(|_| input_error("non-UTF-8 environment name"))?,
                            value
                                .into_string()
                                .map_err(|_| input_error("non-UTF-8 environment value"))?,
                        );
                        bytes = bytes.saturating_add(name.len()).saturating_add(value.len());
                        if bytes > MAX_INPUT_BYTES {
                            return Err(input_error("environment input byte limit"));
                        }
                        environment.insert(name, value);
                    }
                    state.environment = Some(environment);
                }
                InputResponse::Environment(
                    state
                        .environment
                        .clone()
                        .ok_or_else(|| input_error("missing owned environment"))?,
                )
            }
            InputRequest::TextFile(path) if self.grants.files => {
                if state
                    .records
                    .iter()
                    .any(|record| matches!(record.request, InputRequest::TextFile(_)))
                {
                    return Err(input_error(
                        "subsequent TXT Deserialize requests are not implemented (shared native result-name history)",
                    ));
                }
                InputResponse::Text(read_text_file(path)?)
            }
            _ => return Err(input_error("source input category was not granted")),
        };
        let record = InputRecord {
            request: request.clone(),
            response: response.clone(),
        };
        let bytes = state.bytes.saturating_add(record.bytes());
        if bytes > MAX_INPUT_BYTES {
            return Err(input_error("total external input byte limit"));
        }
        state.bytes = bytes;
        state.records.push(record);
        Ok(response)
    }
}

impl InputSource for LiveInputs {
    fn request(&self, request: &InputRequest) -> Result<InputResponse, HostError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        let result = self.read(&mut state, request);
        if let Err(error) = &result {
            state.failure = Some(error.clone());
        }
        result
    }
    fn finish_binding(&self) -> Result<(), HostError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
        state.failure.clone().map_or(Ok(()), Err)
    }
    fn recorded(&self) -> RecordedInputs {
        RecordedInputs {
            grants: self.grants,
            records: self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .records
                .clone(),
        }
    }
}

fn read_text_file(path: &str) -> Result<String, HostError> {
    if path.len() > MAX_INPUT_BYTES {
        return Err(input_error("input path byte limit"));
    }
    if !std::fs::metadata(path)
        .map_err(|_| {
            input_error("IOUtils!Deserialize file-open error records are not implemented")
        })?
        .is_file()
    {
        return Err(input_error("only regular text files are supported"));
    }
    let file = std::fs::File::open(path).map_err(|_| {
        input_error("IOUtils!Deserialize file-open error records are not implemented")
    })?;
    if !file
        .metadata()
        .map_err(|_| input_error("input file metadata"))?
        .is_file()
    {
        return Err(input_error("only regular text files are supported"));
    }
    let mut bytes = Vec::new();
    file.take(u64::try_from(MAX_INPUT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| {
            input_error("IOUtils!Deserialize file-read error records are not implemented")
        })?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(input_error("text input byte limit"));
    }
    String::from_utf8(bytes)
        .map_err(|_| input_error("non-UTF-8 file error records are not implemented"))
}
