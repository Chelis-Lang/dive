//! Managed, producer-only debugging of the existing compiled safety checker.
use crate::{CheckReport, CheckRequest, Engine, check_file_engine};
use dive_core::debug::{Instruction, Observer, Returned, SourceMap};
use dive_core::plan::Code;
use dive_core::{Error, Handle, Machine};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// An invalid, stale, unsupported or failed debugger operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugError(pub String);
impl std::fmt::Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DIVE-D0001: {}", self.0)
    }
}
impl std::error::Error for DebugError {}
fn source_matches(path: &str, loaded: &str) -> bool {
    let (path, loaded) = (std::path::Path::new(path), std::path::Path::new(loaded));
    path == loaded
        || std::env::current_dir()
            .is_ok_and(|directory| directory.join(path) == directory.join(loaded))
}
fn rejected(message: &str) -> DebugError {
    DebugError(message.into())
}

/// An asynchronous event from the actual checker.
#[derive(Clone, Debug)]
pub enum Event {
    /// The compiled map is available; configuration still gates search.
    Prepared,
    /// Actual producer execution is suspended with stop-scoped handles.
    Stopped {
        /// Monotonic stop identity.
        id: u64,
        /// `breakpoint`, `step`, `pause`, `entry` or `exception`.
        reason: String,
        /// Actual matched source-breakpoint identities.
        breakpoints: Vec<u64>,
        /// Actual exception/violation description when present.
        description: Option<String>,
    },
    /// The ordinary checker has returned this report, including refusals/stops.
    Completed(Value),
    /// Loading or the host operation failed without a checker result.
    Failed(String),
}

/// A requested source breakpoint, verified only against live compiled instructions.
#[derive(Clone, Debug)]
pub struct Breakpoint {
    /// Stable identity for this installed breakpoint.
    pub id: u64,
    /// Exact one-based source line.
    pub line: u32,
    /// Whether an executable instruction begins on this source line.
    pub verified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    file: usize,
    line: u32,
    environment: usize,
    definition: usize,
    instance: Option<u32>,
    base: usize,
}
#[derive(Clone, Debug)]
struct Activation {
    instruction: Instruction,
    base: usize,
}
#[derive(Clone, Copy, Debug)]
enum Motion {
    Continue,
    Into,
    Over(usize),
    Out(usize),
}
#[derive(Clone, Debug)]
struct Installed {
    file: usize,
    breakpoint: Breakpoint,
}
#[derive(Clone, Debug)]
struct Frame {
    id: u64,
    document: Value,
    state: Vec<Value>,
    next: Vec<Value>,
    locals: Vec<Value>,
}
#[derive(Clone, Debug)]
struct Snapshot {
    frames: Vec<Frame>,
    key: Key,
    depth: usize,
}
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent protocol controls, motion, error-unwind and producer lifecycle flags"
)]
struct State {
    map: Option<SourceMap>,
    configured: bool,
    terminated: bool,
    stop_entry: bool,
    pause: bool,
    exceptions: bool,
    motion: Motion,
    compat_tlc: bool,
    stack: Vec<Activation>,
    breakpoints: Vec<Installed>,
    next_breakpoint: u64,
    ignore: Option<(Key, usize)>,
    snapshot: Option<Snapshot>,
    epoch: u64,
    events: VecDeque<Event>,
    completion: Option<Value>,
    error_in_flight: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            map: None,
            configured: false,
            terminated: false,
            stop_entry: false,
            pause: false,
            exceptions: false,
            motion: Motion::Continue,
            compat_tlc: false,
            stack: Vec::new(),
            breakpoints: Vec::new(),
            next_breakpoint: 1,
            ignore: None,
            snapshot: None,
            epoch: 0,
            events: VecDeque::new(),
            completion: None,
            error_in_flight: false,
        }
    }
}
#[derive(Debug, Default)]
struct Control {
    state: Mutex<State>,
    changed: Condvar,
}
impl Control {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn key(map: &SourceMap, instruction: Instruction, base: usize) -> Option<Key> {
        let location = map.location(instruction)?;
        Some(Key {
            file: location.origin.file?.index(),
            line: location.origin.begin.0,
            environment: location.environment_id,
            definition: location.definition_id,
            instance: location.substitutions.last().map(|entry| entry.identity),
            base,
        })
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "atomic producer stop retains lock, actual instruction/machine and event details"
    )]
    fn stop<'a>(
        &self,
        mut state: MutexGuard<'a, State>,
        instruction: Instruction,
        code: &Code,
        machine: &Machine,
        reason: &str,
        breakpoints: Vec<u64>,
        description: Option<String>,
    ) -> Result<MutexGuard<'a, State>, Error> {
        state.epoch = state
            .epoch
            .checked_add(1)
            .filter(|epoch| *epoch < 524_287)
            .ok_or_else(|| Error::unsupported("debugger stop handle capacity"))?;
        let snapshot = snapshot(&state, instruction, code, machine)?;
        let id = state.epoch;
        state.snapshot = Some(snapshot);
        if state.events.len() >= 256 {
            return Err(Error::unsupported("debugger unconsumed event capacity"));
        }
        state.events.push_back(Event::Stopped {
            id,
            reason: reason.into(),
            breakpoints,
            description,
        });
        self.changed.notify_all();
        while state.snapshot.is_some() && !state.terminated {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if state.terminated {
            return Err(Error::unsupported(
                "debugger terminated the check before completion",
            ));
        }
        Ok(state)
    }
    fn complete(&self, report: Result<&CheckReport, String>) {
        let mut state = self.lock();
        state.snapshot = None;
        match report {
            Ok(report) => {
                let document = report.to_json_with_exit_convention(crate::ExitConvention::Native);
                state.completion = Some(document.clone());
                state.events.push_back(Event::Completed(document));
            }
            Err(error) => state.events.push_back(Event::Failed(error)),
        }
        self.changed.notify_all();
    }
}
impl Observer for Control {
    fn prepared(&self, map: &SourceMap) -> Result<(), Error> {
        let mut state = self.lock();
        state.map = Some(map.clone());
        state.events.push_back(Event::Prepared);
        self.changed.notify_all();
        while !state.configured && !state.terminated {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if state.terminated {
            Err(Error::unsupported("debugger terminated before search"))
        } else {
            Ok(())
        }
    }
    fn enter(&self, instruction: Instruction, code: &Code, machine: &Machine) -> Result<(), Error> {
        let mut state = self.lock();
        if state.terminated {
            return Err(Error::unsupported(
                "debugger terminated the check before completion",
            ));
        }
        if state.stack.len() >= 4096 {
            return Err(Error::unsupported("debugger activation capacity"));
        }
        state.stack.push(Activation {
            instruction,
            base: machine.base,
        });
        let Some(map) = &state.map else {
            return Ok(());
        };
        let Some(key) = Self::key(map, instruction, machine.base) else {
            return Ok(());
        };
        let depth = state.stack.len();
        let hits = state
            .breakpoints
            .iter()
            .filter(|bp| {
                bp.breakpoint.verified && bp.file == key.file && bp.breakpoint.line == key.line
            })
            .map(|bp| bp.breakpoint.id)
            .collect::<Vec<_>>();
        let ignored = state.ignore.is_some_and(|(old, _)| old == key);
        let reason = if state.pause {
            Some("pause")
        } else if state.stop_entry {
            Some("entry")
        } else if !hits.is_empty() && !ignored {
            Some("breakpoint")
        } else if match state.motion {
            Motion::Continue => false,
            Motion::Into => true,
            Motion::Over(old) => depth <= old,
            Motion::Out(old) => depth < old,
        } {
            Some("step")
        } else {
            None
        };
        if let Some(reason) = reason {
            state.pause = false;
            state.stop_entry = false;
            state.motion = Motion::Continue;
            if let Err(error) = self.stop(state, instruction, code, machine, reason, hits, None) {
                self.lock().stack.pop();
                return Err(error);
            }
        }
        Ok(())
    }
    fn returned(
        &self,
        instruction: Instruction,
        code: &Code,
        machine: &Machine,
        result: Returned<'_>,
    ) -> Result<(), Error> {
        let mut state = self.lock();
        if state.terminated {
            return Err(Error::unsupported(
                "debugger terminated the check before completion",
            ));
        }
        if !state.exceptions || state.error_in_flight {
            return Ok(());
        }
        let description = match result {
            Returned::Failed(error) if error.kind() == dive_core::ErrorKind::Eval => {
                Some(error.message().to_owned())
            }
            Returned::Value(value) if value.as_bool() == Some(false) => {
                if let Instruction::Value(index) = instruction {
                    state
                        .map
                        .as_ref()
                        .and_then(|map| map.obligations.iter().find(|(node, _)| *node == index))
                        .map(|(_, name)| format!("Property {name} is false"))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(description) = description
            && state
                .map
                .as_ref()
                .and_then(|map| Self::key(map, instruction, machine.base))
                .is_some()
        {
            state.error_in_flight = true;
            drop(self.stop(
                state,
                instruction,
                code,
                machine,
                "exception",
                Vec::new(),
                Some(description),
            )?);
        }
        Ok(())
    }
    fn leave(&self) {
        let mut state = self.lock();
        state.stack.pop();
        if state
            .ignore
            .is_some_and(|(_, depth)| state.stack.len() < depth)
        {
            state.ignore = None;
        }
        if state.stack.is_empty() {
            state.error_in_flight = false;
        }
    }
}
fn scalar(value: Handle, machine: &Machine) -> (String, &'static str, Option<Value>) {
    if let Some(value) = value.as_int() {
        return (value.to_string(), "integer", Some(json!(value)));
    }
    if let Some(value) = value.as_bool() {
        return (
            if value { "TRUE" } else { "FALSE" }.into(),
            "boolean",
            Some(json!(value)),
        );
    }
    if let Some(id) = value.as_str_id()
        && let Some(text) = machine.heap.store().strings().text(id)
        && text.len() <= 4096
    {
        return (json!(&*text).to_string(), "string", Some(json!(&*text)));
    }
    if value.is_empty() {
        ("<unassigned or unforced>".into(), "unavailable", None)
    } else {
        ("<opaque; not inspected>".into(), "unavailable", None)
    }
}
fn variable(name: &str, value: Option<Handle>, machine: &Machine) -> Value {
    let (text, kind, scalar) = value.map_or_else(
        || ("<not evaluated>".into(), "unavailable", None),
        |value| scalar(value, machine),
    );
    json!({"name":name,"value":text,"type":kind,"variablesReference":0,"scalar":scalar})
}
fn dap_position(source: &str, offset: u32) -> Option<(u32, u32)> {
    let prefix = source.get(..usize::try_from(offset).ok()?)?;
    let mut characters = prefix.chars().peekable();
    let (mut line, mut column) = (1_u32, 1_u32);
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                line += 1;
                column = 1;
            }
            '\n' => {
                line += 1;
                column = 1;
            }
            _ => column += u32::try_from(character.len_utf16()).ok()?,
        }
    }
    Some((line, column))
}

fn snapshot(
    state: &State,
    instruction: Instruction,
    code: &Code,
    machine: &Machine,
) -> Result<Snapshot, Error> {
    let map = state
        .map
        .as_ref()
        .ok_or_else(|| Error::internal("debugger map unavailable"))?;
    let key = Control::key(map, instruction, machine.base)
        .ok_or_else(|| Error::internal("debugger stop without source"))?;
    let mut frames = Vec::new();
    let mut previous = None;
    for activation in state.stack.iter().rev() {
        let Some(location) = map.location(activation.instruction) else {
            continue;
        };
        let Some(file) = location
            .origin
            .file
            .and_then(|file| map.files.get(file.index()))
        else {
            continue;
        };
        let identity = (
            location.definition_id,
            location.environment_id,
            activation.base,
            location.substitutions.last().map(|entry| entry.identity),
        );
        if previous == Some(identity) {
            continue;
        }
        previous = Some(identity);
        if frames.len() >= 128 {
            return Err(Error::unsupported("debugger visible frame capacity"));
        }
        let ordinal =
            u64::try_from(frames.len()).map_err(|_| Error::internal("debugger frame index"))?;
        let id = state.epoch * 1024 + ordinal + 1;
        let begin = dap_position(&file.source, location.origin.range.0)
            .ok_or_else(|| Error::internal("compiled debugger source start range"))?;
        let end = dap_position(&file.source, location.origin.range.1)
            .ok_or_else(|| Error::internal("compiled debugger source end range"))?;
        let document = json!({"id":id,"name":location.definition,"line":begin.0,"column":begin.1,
            "endLine":end.0,"endColumn":end.1,
            "source":{"name":file.module,"path":file.display,"sourceReference":location.origin.file.map(|file|file.index()+1)},
            "instruction":format!("{:?}",activation.instruction),"instance":location.origin.instance.map(dive_ir::DefId::index),"environment":location.environment_id,"compilationCaller":location.caller,
            "substitutions":location.substitutions.iter().map(|entry|json!({"declaration":entry.declaration,"identity":entry.identity,
                "actual":entry.actual.map(|origin|json!({"path":origin.file.and_then(|file|map.files.get(file.index())).map(|file|&file.display),"begin":origin.begin,"end":origin.end,"range":origin.range}))})).collect::<Vec<_>>()});
        let locals = location
            .locals
            .iter()
            .map(|local| {
                variable(
                    &local.name,
                    dive_core::debug::read_binding(&local.binding, code, machine, activation.base),
                    machine,
                )
            })
            .collect();
        let values = |values: &[Handle]| {
            map.variables
                .iter()
                .enumerate()
                .map(|(index, name)| variable(name, values.get(index).copied(), machine))
                .collect()
        };
        frames.push(Frame {
            id,
            document,
            state: values(&machine.s0),
            next: values(&machine.s1),
            locals,
        });
    }
    Ok(Snapshot {
        frames,
        key,
        depth: state.stack.len(),
    })
}

/// A running local one-worker Core check and its stop-scoped inspection capability.
///
/// ```no_run
/// use dive_api::{CheckRequest, debug::{Session, Event}};
/// let mut session = Session::launch(CheckRequest { file:"Spec.tla".into(), ..Default::default() }, false)?;
/// session.wait_event(std::time::Duration::from_secs(10))?;
/// session.configuration_done()?;
/// while let Some(event) = session.wait_event(std::time::Duration::from_secs(10))? {
///     if matches!(event, Event::Completed(_)) { break; }
/// }
/// let report = session.finish()?;
/// # let _ = report;
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Session {
    control: Arc<Control>,
    worker: Option<JoinHandle<Result<CheckReport, crate::ApiError>>>,
}
impl Session {
    /// Starts loading/compiling with existing checking APIs. Search waits for configuration.
    /// The report retains the requested worker width; observed search uses one
    /// actual worker.
    ///
    /// # Errors
    /// An unsupported managed-session combination or failed thread creation.
    pub fn launch(mut request: CheckRequest, stop_on_entry: bool) -> Result<Self, DebugError> {
        if request.store_compact_bytes.is_some() {
            return Err(rejected(
                "compiled debugger with Store reclamation is not implemented",
            ));
        }
        if request.tlc_coverage {
            return Err(rejected(
                "compiled debugger with TLC logical coverage is not implemented",
            ));
        }
        if request.source_coverage {
            return Err(rejected(
                "compiled debugger with native source coverage is not implemented",
            ));
        }
        if request.distributed.is_some() {
            return Err(rejected(
                "compiled debugger with distributed safety checking is not implemented",
            ));
        }
        if request.debugger.is_some()
            || request.temporal != crate::TemporalMode::Ordinary
            || request.core_sample.is_some()
            || !request.host_access.is_empty()
        {
            return Err(rejected(
                "debugger supports local Core ordinary safety without host grants, coverage or sampling",
            ));
        }
        let control = Arc::new(Control::default());
        {
            let mut state = control.lock();
            state.stop_entry = stop_on_entry;
            state.compat_tlc = request.compat_tlc;
        }
        request.debugger = Some(Arc::clone(&control) as Arc<dyn Observer>);
        let observed = Arc::clone(&control);
        let worker = std::thread::Builder::new()
            .name("dive-debug-check".into())
            .stack_size(32 << 20)
            .spawn(move || {
                let result = check_file_engine(&request, Engine::Core);
                observed.complete(result.as_ref().map_err(ToString::to_string));
                result
            })
            .map_err(|error| DebugError(error.to_string()))?;
        Ok(Self {
            control,
            worker: Some(worker),
        })
    }
    /// Waits at most the given duration for a real producer event.
    ///
    /// # Errors
    /// A wait above one minute is rejected.
    pub fn wait_event(&self, timeout: Duration) -> Result<Option<Event>, DebugError> {
        if timeout > Duration::from_secs(60) {
            return Err(rejected("debugger event wait exceeds one minute"));
        }
        let deadline = Instant::now() + timeout;
        let mut state = self.control.lock();
        loop {
            if let Some(event) = state.events.pop_front() {
                return Ok(Some(event));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            let (next, wait) = self
                .control
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            state = next;
            if wait.timed_out() {
                return Ok(state.events.pop_front());
            }
        }
    }
    /// Installs exact-line source breakpoints using only already-loaded sources.
    ///
    /// # Errors
    /// Unknown source, invalid/capacity line request or unavailable compilation.
    pub fn set_breakpoints(
        &self,
        path: &str,
        lines: &[u32],
    ) -> Result<Vec<Breakpoint>, DebugError> {
        if lines.len() > 1024 || lines.contains(&0) {
            return Err(rejected("invalid or excessive source breakpoints"));
        }
        let mut state = self.control.lock();
        let map = state
            .map
            .as_ref()
            .ok_or_else(|| rejected("compiled map is not ready"))?;
        let file = map
            .files
            .iter()
            .position(|file| source_matches(path, &file.display))
            .ok_or_else(|| rejected("source is not loaded by this check"))?;
        let verified = lines
            .iter()
            .map(|line| {
                map.executable.iter().any(|site| {
                    map.location(*site).is_some_and(|location| {
                        location.origin.file.is_some_and(|id| id.index() == file)
                            && location.origin.begin.0 == *line
                    })
                })
            })
            .collect::<Vec<_>>();
        state.breakpoints.retain(|bp| bp.file != file);
        let mut result = Vec::new();
        for (line, verified) in lines.iter().zip(verified) {
            let id = state.next_breakpoint;
            state.next_breakpoint = state
                .next_breakpoint
                .checked_add(1)
                .filter(|id| *id < 1_000_000_000)
                .ok_or_else(|| rejected("breakpoint identity capacity"))?;
            let breakpoint = Breakpoint {
                id,
                line: *line,
                verified,
            };
            state.breakpoints.push(Installed {
                file,
                breakpoint: breakpoint.clone(),
            });
            result.push(breakpoint);
        }
        Ok(result)
    }
    /// Configures an entry stop before the pending producer search starts.
    ///
    /// # Errors
    /// Search has started or the session is unavailable.
    pub fn break_on_entry(&self, enabled: bool) -> Result<(), DebugError> {
        let mut state = self.control.lock();
        if state.configured || state.terminated || state.completion.is_some() {
            return Err(rejected("entry policy requires a pending check"));
        }
        state.stop_entry = enabled;
        Ok(())
    }

    /// Starts actual search after breakpoint/configuration requests.
    ///
    /// # Errors
    /// Compilation is unavailable or the session has terminated.
    pub fn configuration_done(&self) -> Result<(), DebugError> {
        let mut state = self.control.lock();
        if state.map.is_none() || state.terminated || state.configured {
            return Err(rejected("debugger configuration is unavailable"));
        }
        state.configured = true;
        self.control.changed.notify_all();
        Ok(())
    }
    /// Selects actual evaluation-error/property-failure stops.
    ///
    /// # Errors
    /// The compiled session is unavailable or complete.
    pub fn exception_breakpoints(&self, enabled: bool) -> Result<(), DebugError> {
        let mut state = self.control.lock();
        if state.map.is_none() || state.terminated || state.completion.is_some() {
            return Err(rejected("no active compiled session"));
        }
        state.exceptions = enabled;
        Ok(())
    }
    /// Requests a stop at the next mapped producer instruction.
    ///
    /// # Errors
    /// Search is not active.
    pub fn pause(&self) -> Result<(), DebugError> {
        let mut state = self.control.lock();
        if !state.configured
            || state.completion.is_some()
            || state.terminated
            || state.snapshot.is_some()
        {
            return Err(rejected("no active producer to pause"));
        }
        state.pause = true;
        Ok(())
    }
    fn resume(&self, motion: Motion) -> Result<(), DebugError> {
        let mut state = self.control.lock();
        let snapshot = state
            .snapshot
            .take()
            .ok_or_else(|| rejected("producer is not stopped"))?;
        state.ignore = Some((snapshot.key, snapshot.depth));
        state.motion = match motion {
            Motion::Over(_) => Motion::Over(snapshot.depth),
            Motion::Out(_) => Motion::Out(snapshot.depth),
            other => other,
        };
        self.control.changed.notify_all();
        Ok(())
    }
    /// Continues actual execution and expires all stop handles.
    ///
    /// # Errors
    /// The producer is not stopped.
    pub fn continue_check(&self) -> Result<(), DebugError> {
        self.resume(Motion::Continue)
    }
    /// Stops at the next mapped instruction, including nested evaluation.
    ///
    /// # Errors
    /// The producer is not stopped.
    pub fn step_in(&self) -> Result<(), DebugError> {
        self.resume(Motion::Into)
    }
    /// Steps over nested instruction activations.
    ///
    /// # Errors
    /// The producer is not stopped.
    pub fn next(&self) -> Result<(), DebugError> {
        self.resume(Motion::Over(0))
    }
    /// Runs until an enclosing actual activation is reached.
    ///
    /// # Errors
    /// The producer is not stopped.
    pub fn step_out(&self) -> Result<(), DebugError> {
        self.resume(Motion::Out(0))
    }
    /// Returns the current actual source/activation stack.
    ///
    /// # Errors
    /// The producer is not stopped.
    pub fn stack(&self) -> Result<Vec<Value>, DebugError> {
        let state = self.control.lock();
        Ok(state
            .snapshot
            .as_ref()
            .ok_or_else(|| rejected("producer is not stopped"))?
            .frames
            .iter()
            .map(|frame| frame.document.clone())
            .collect())
    }
    /// Returns state, successor and local scope handles for a current frame.
    ///
    /// # Errors
    /// The frame handle is stale/unknown or the producer is running.
    pub fn scopes(&self, frame: u64) -> Result<Vec<Value>, DebugError> {
        let state = self.control.lock();
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or_else(|| rejected("producer is not stopped"))?;
        let frame = snapshot
            .frames
            .iter()
            .find(|candidate| candidate.id == frame)
            .ok_or_else(|| rejected("stale or unknown frame handle"))?;
        Ok([("State",1),("Successor",2),("Locals",3)].map(|(name,kind)|json!({"name":name,"variablesReference":frame.id*4+kind,"expensive":false})).to_vec())
    }
    /// Reads a current scope's captured values without evaluating source.
    ///
    /// # Errors
    /// The scope handle is stale/unknown or the producer is running.
    pub fn variables(&self, reference: u64) -> Result<Vec<Value>, DebugError> {
        let state = self.control.lock();
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or_else(|| rejected("producer is not stopped"))?;
        for frame in &snapshot.frames {
            let values = match reference.checked_sub(frame.id * 4) {
                Some(1) => Some(&frame.state),
                Some(2) => Some(&frame.next),
                Some(3) => Some(&frame.locals),
                _ => None,
            };
            if let Some(values) = values {
                return Ok(values.clone());
            }
        }
        Err(rejected("stale or unknown variables handle"))
    }
    /// Evaluates bounded integer/boolean arithmetic in a fresh compiler/machine
    /// over captured scalars. No producer recipe or source helper is executed.
    ///
    /// # Errors
    /// A stale frame, unavailable operand, unsupported syntax or work bound.
    pub fn evaluate(&self, frame: u64, expression: &str) -> Result<Value, DebugError> {
        let (values, compat_tlc) = {
            let state = self.control.lock();
            let snapshot = state
                .snapshot
                .as_ref()
                .ok_or_else(|| rejected("producer is not stopped"))?;
            let frame = snapshot
                .frames
                .iter()
                .find(|candidate| candidate.id == frame)
                .ok_or_else(|| rejected("stale or unknown frame handle"))?;
            let mut values = HashMap::new();
            for value in frame.state.iter().chain(&frame.locals) {
                if let Some(name) = value["name"].as_str() {
                    values.insert(name.to_owned(), value["scalar"].clone());
                }
            }
            (values, state.compat_tlc)
        };
        evaluate_scalar(expression, &values, compat_tlc)
    }

    /// Returns only a source already loaded by this session, without opening a path.
    ///
    /// # Errors
    /// The loaded-source reference is invalid or unavailable.
    pub fn source(&self, reference: usize) -> Result<String, DebugError> {
        let state = self.control.lock();
        let map = state
            .map
            .as_ref()
            .ok_or_else(|| rejected("compiled map is not ready"))?;
        Ok(map
            .files
            .get(
                reference
                    .checked_sub(1)
                    .ok_or_else(|| rejected("invalid source reference"))?,
            )
            .ok_or_else(|| rejected("unknown source reference"))?
            .source
            .to_string())
    }
    /// Requests termination; incomplete work never yields a success verdict.
    pub fn terminate(&self) {
        let mut state = self.control.lock();
        state.terminated = true;
        state.snapshot = None;
        self.control.changed.notify_all();
    }
    /// Joins the existing checker and returns its ordinary typed report.
    ///
    /// # Errors
    /// A host failure or repeated finish request.
    pub fn finish(&mut self) -> Result<CheckReport, DebugError> {
        if !self
            .worker
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && self.control.lock().completion.is_none()
        {
            return Err(rejected(
                "checker has not returned; continue or terminate before joining",
            ));
        }
        let worker = self
            .worker
            .take()
            .ok_or_else(|| rejected("checker already joined"))?;
        worker
            .join()
            .map_err(|_| rejected("debugger checking thread failed"))?
            .map_err(|error| DebugError(error.to_string()))
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.terminate();
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "admit bounded captured scalar grammar before isolated native compile/evaluation"
)]
fn evaluate_scalar(
    expression: &str,
    values: &HashMap<String, Value>,
    compat_tlc: bool,
) -> Result<Value, DebugError> {
    use dive_core::compile::{Handoff, Role};
    if expression.is_empty() || expression.len() > 1024 {
        return Err(rejected("debugger expression byte bound"));
    }
    let operators = [
        "+", "-", "*", "\\div", "%", "=", "#", "/=", "<", ">", "<=", ">=", "/\\", "\\/", "~", "=>",
        "<=>", "(", ")", "TRUE", "FALSE",
    ];
    let tokens = dive_syntax::tokens::tokenize_spec(expression);
    if tokens.len() > 256 {
        return Err(rejected("debugger expression token bound"));
    }
    for token in &tokens {
        if token.error.is_some() {
            return Err(rejected("invalid debugger expression token"));
        }
        if token.kind.is_trivia() {
            continue;
        }
        let word = &expression[token.range];
        let integer = word.len() <= 10 && word.bytes().all(|byte| byte.is_ascii_digit());
        let known = values
            .get(word)
            .is_some_and(|value| value.is_i64() || value.is_boolean());
        if !integer && !known && !operators.contains(&word) {
            return Err(rejected(
                "debugger evaluation admits only captured integer/boolean names and bounded pure operators",
            ));
        }
    }
    let mut source = String::from("---- MODULE DebuggerExpression ----\nEXTENDS Integers\n");
    let mut names = values
        .keys()
        .filter(|name| {
            tokens
                .iter()
                .any(|token| &expression[token.range] == name.as_str())
        })
        .collect::<Vec<_>>();
    names.sort();
    for name in names {
        let value = &values[name];
        let literal = if let Some(value) = value.as_i64() {
            value.to_string()
        } else if let Some(value) = value.as_bool() {
            if value { "TRUE" } else { "FALSE" }.into()
        } else {
            continue;
        };
        let _ = writeln!(source, "{name} == {literal}");
    }
    let mut result_name = String::from("DIVE_debug_result");
    while values.contains_key(&result_name) {
        result_name.push('_');
    }
    let _ = write!(source, "{result_name} == ({expression})\n====\n");
    let analysis = dive_sema::analyze_source(
        "DebuggerExpression.tla",
        &source,
        &dive_sema::Options::default(),
    );
    let ir = dive_ir::lower(&analysis).map_err(|error| DebugError(error.to_string()))?;
    let definition = ir
        .root_def(&result_name)
        .ok_or_else(|| rejected("debugger expression definition"))?;
    let eval = if compat_tlc {
        dive_eval::Options::tlc()
    } else {
        dive_eval::Options::dive()
    };
    let store = Arc::new(dive_core::Store::with_semantics(
        true,
        crate::core_engine::semantics(&eval),
    ));
    let h = Handoff {
        ir: &ir,
        ctx: eval.ctx,
        action_composition: eval.action_composition == dive_eval::ActionComposition::Tla,
        constants: Vec::new(),
        redirects: HashMap::new(),
        std_redirects: HashMap::new(),
        precomputed: HashMap::new(),
        overrides: std::collections::HashSet::new(),
        deferred: std::collections::BTreeSet::new(),
        startup_evaluator: None,
        init: Vec::new(),
        next: Vec::new(),
        invariants: vec![Role {
            name: result_name,
            expr: ir.def(definition).body,
            def: definition,
            slots: Vec::new(),
            substs: Vec::new(),
        }],
        constraints: Vec::new(),
        action_constraints: Vec::new(),
        implied_inits: Vec::new(),
        implied_actions: Vec::new(),
        check_deadlock: false,
        symmetry: Vec::new(),
        view: None,
    };
    let opts = dive_core::Opts {
        fold: false,
        memo: false,
        step_memo: false,
        primed_memo: false,
        ..dive_core::Opts::default()
    };
    let plan = dive_core::compile(&h, Arc::clone(&store), opts)
        .map_err(|error| DebugError(error.to_string()))?;
    let check = plan
        .invariants
        .first()
        .ok_or_else(|| rejected("debugger expression plan"))?;
    let part = check
        .conjuncts
        .first()
        .ok_or_else(|| rejected("debugger expression root"))?;
    let mut machine = Machine::new(store, 0, opts, dive_core::memo::Memo::default());
    machine.set_fuel(4096);
    let old = machine.enter(check.frame);
    let result = machine
        .eval(&plan.code, part.p)
        .map_err(|error| DebugError(error.to_string()));
    machine.leave(old);
    let value = result?;
    let (text, kind, scalar) = scalar(value, &machine);
    if scalar.is_none() {
        return Err(rejected(
            "debugger expression produced an unproved scalar representation",
        ));
    }
    Ok(json!({"result":text,"type":kind,"variablesReference":0,"scalar":scalar}))
}
