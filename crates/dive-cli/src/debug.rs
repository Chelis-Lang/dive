//! Framed native DAP transport for the actual compiled-check session.
use dive_api::{
    CheckRequest,
    debug::{Event, Session},
};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

fn boolean(args: &Value, name: &str, default: bool) -> Result<bool, String> {
    args.get(name).map_or(Ok(default), |value| {
        value
            .as_bool()
            .ok_or_else(|| format!("invalid boolean {name}"))
    })
}
fn unsigned(args: &Value, name: &str, default: u64) -> Result<u64, String> {
    args.get(name).map_or(Ok(default), |value| {
        value
            .as_u64()
            .ok_or_else(|| format!("invalid unsigned {name}"))
    })
}
fn frame(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut size = None;
    let mut used = 0;
    loop {
        let mut line = Vec::new();
        let count = reader
            .by_ref()
            .take(8193_u64)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return if used == 0 {
                Ok(None)
            } else {
                Err("incomplete DAP header".into())
            };
        }
        used += count;
        if used > 8192 || !line.ends_with(b"\n") {
            return Err("DAP header byte bound".into());
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        let line = std::str::from_utf8(&line)
            .map_err(|_| "non-UTF8 DAP header")?
            .trim_end_matches(['\r', '\n']);
        let (name, value) = line.split_once(':').ok_or("malformed DAP header")?;
        if name.eq_ignore_ascii_case("Content-Length") {
            if size.is_some() {
                return Err("duplicate Content-Length".into());
            }
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err("invalid Content-Length".into());
            }
            let count = value
                .parse::<usize>()
                .map_err(|_| "Content-Length overflow")?;
            if count == 0 || count > 1 << 20 {
                return Err("DAP frame byte bound".into());
            }
            size = Some(count);
        } else if !name.eq_ignore_ascii_case("Content-Type") {
            return Err("unsupported DAP header".into());
        }
    }
    let mut bytes = vec![0; size.ok_or("missing Content-Length")?];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let message: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid DAP JSON: {e}"))?;
    if message["type"] != "request"
        || message["seq"]
            .as_u64()
            .is_none_or(|seq| seq == 0 || seq > i32::MAX.cast_unsigned().into())
        || !message["command"].is_string()
    {
        return Err("invalid DAP request envelope".into());
    }
    Ok(Some(message))
}
struct Wire<'a, W: Write> {
    output: &'a mut W,
    sequence: u64,
}
impl<W: Write> Wire<'_, W> {
    fn send(&mut self, mut message: Value) -> Result<(), String> {
        if self.sequence >= i32::MAX.cast_unsigned().into() {
            return Err("DAP message sequence capacity".into());
        }
        self.sequence += 1;
        message["seq"] = json!(self.sequence);
        let bytes = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
        if bytes.len() > 8 << 20 {
            return Err("DAP output byte bound".into());
        }
        write!(self.output, "Content-Length: {}\r\n\r\n", bytes.len())
            .map_err(|e| e.to_string())?;
        self.output.write_all(&bytes).map_err(|e| e.to_string())?;
        self.output.flush().map_err(|e| e.to_string())
    }
    fn response(&mut self, request: &Value, result: Result<Value, String>) -> Result<(), String> {
        self.send(match result{Ok(body)=>json!({"type":"response","request_seq":request["seq"],"command":request["command"],"success":true,"body":body}),Err(message)=>json!({"type":"response","request_seq":request["seq"],"command":request["command"],"success":false,"message":format!("DIVE-D0001: {message}")})})
    }
    fn event(&mut self, name: &str, body: &Value) -> Result<(), String> {
        self.send(json!({"type":"event","event":name,"body":body}))
    }
}
#[derive(Default)]
struct ClientCapabilities {
    variable_types: bool,
    paging: bool,
}
#[allow(
    clippy::struct_excessive_bools,
    reason = "separate client initialization, binding kind and transport completion controls"
)]
struct Server {
    capabilities: ClientCapabilities,
    request: CheckRequest,
    session: Option<Session>,
    initialized: bool,
    attached: bool,
    attached_existing: bool,
    last_request: u64,
    last_exception: Option<String>,
    pending: Option<Event>,
    done: bool,
}
impl Server {
    fn active(&self) -> Result<&Session, String> {
        if !self.attached {
            return Err("no launched/attached checker".into());
        }
        self.session
            .as_ref()
            .ok_or_else(|| "no launched/attached checker".into())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per explicitly supported DAP request and refusal boundary"
    )]
    fn request(&mut self, message: &Value) -> Result<Value, String> {
        let sequence = message["seq"].as_u64().ok_or("invalid request sequence")?;
        if sequence != self.last_request + 1 {
            self.last_request = self.last_request.max(sequence);
            return Err("stale, repeated or nonconsecutive DAP request sequence".into());
        }
        self.last_request = sequence;
        let command = message["command"].as_str().ok_or("invalid command")?;
        let args = &message["arguments"];
        if !args.is_null() && !args.is_object() {
            return Err("DAP arguments must be an object".into());
        }
        if command == "initialize" {
            if self.initialized {
                return Err("DAP already initialized".into());
            }
            if args["adapterID"].as_str().is_none_or(str::is_empty) {
                return Err("initialize requires adapterID".into());
            }
            if !boolean(args, "linesStartAt1", true)? || !boolean(args, "columnsStartAt1", true)? {
                return Err("one-based source coordinates are required".into());
            }
            if args
                .get("pathFormat")
                .is_some_and(|format| format != "path")
            {
                return Err("only loaded source paths are supported".into());
            }
            self.capabilities.variable_types = boolean(args, "supportsVariableType", false)?;
            self.capabilities.paging = boolean(args, "supportsVariablePaging", false)?;
            self.initialized = true;
            return Ok(
                json!({"supportsConfigurationDoneRequest":true,"supportsTerminateRequest":true,"supportsEvaluateForHovers":true,"supportsExceptionInfoRequest":true,"exceptionBreakpointFilters":[{"filter":"all","label":"Evaluation errors and false safety properties","default":false}]}),
            );
        }
        if !self.initialized {
            return Err("initialize is required".into());
        }
        let id = |name: &str| {
            args[name]
                .as_u64()
                .ok_or_else(|| format!("missing/invalid {name}"))
        };
        let thread = || {
            if args["threadId"].as_u64() == Some(1) {
                Ok(())
            } else {
                Err("threadId 1 is required".to_owned())
            }
        };
        match command {
            "launch" | "attach" => {
                if self.attached {
                    return Err("checker already launched/attached".into());
                }
                if args.as_object().is_some_and(|object| {
                    object.keys().any(|key| {
                        ![
                            "program",
                            "stopOnEntry",
                            "noDebug",
                            "name",
                            "type",
                            "request",
                        ]
                        .contains(&key.as_str())
                    })
                }) {
                    return Err("unsupported launch/attach argument; remote/process/host options are not admitted".into());
                }
                if (command == "attach") != self.session.is_some() {
                    return Err("attach requires the existing check on a loopback listener; launch requires stdio".into());
                }
                if let Some(program) = args.get("program") {
                    let program = program.as_str().ok_or("invalid program path")?;
                    if std::path::Path::new(program) != self.request.file {
                        return Err("program is outside the CLI-authorized check".into());
                    }
                }
                if boolean(args, "noDebug", false)?
                    || args.get("cwd").is_some()
                    || args.get("env").is_some()
                    || args.get("args").is_some()
                {
                    return Err(
                        "DAP cannot grant extra process/environment/argument privileges".into(),
                    );
                }
                let entry = boolean(args, "stopOnEntry", false)?;
                if command == "launch" {
                    self.session = Some(
                        Session::launch(self.request.clone(), entry)
                            .map_err(|error| error.to_string())?,
                    );
                } else if let Some(session) = &self.session {
                    session
                        .break_on_entry(entry)
                        .map_err(|error| error.to_string())?;
                }
                self.attached = true;
                self.attached_existing = command == "attach";
                match self
                    .active()?
                    .wait_event(Duration::from_secs(30))
                    .map_err(|e| e.to_string())?
                {
                    Some(Event::Prepared) => Ok(json!({})),
                    Some(other) => {
                        self.pending = Some(other);
                        Err("check has no compiled debugging session".into())
                    }
                    None => {
                        self.active()?.terminate();
                        Err("debugger preparation wait bound".into())
                    }
                }
            }
            "setBreakpoints" => {
                if boolean(args, "sourceModified", false)? {
                    return Err("modified sources require a new checking session".into());
                }
                let path = args["source"]["path"]
                    .as_str()
                    .ok_or("loaded source path is required")?;
                let mut lines = Vec::new();
                for bp in args["breakpoints"]
                    .as_array()
                    .ok_or("breakpoints array is required")?
                {
                    if ["condition", "hitCondition", "logMessage", "column"]
                        .iter()
                        .any(|key| bp.get(*key).is_some())
                    {
                        return Err(
                            "conditional, hit-count, log and column breakpoints are unsupported"
                                .into(),
                        );
                    }
                    lines.push(
                        u32::try_from(bp["line"].as_u64().ok_or("invalid breakpoint line")?)
                            .map_err(|_| "breakpoint line overflow")?,
                    );
                }
                let breakpoints = self
                    .active()?
                    .set_breakpoints(path, &lines)
                    .map_err(|e| e.to_string())?;
                Ok(
                    json!({"breakpoints":breakpoints.iter().map(|bp|json!({"id":bp.id,"line":bp.line,"verified":bp.verified,"message":if bp.verified{"compiled executable instruction"}else{"no executable instruction starts here; source may be folded, aliased or eliminated"}})).collect::<Vec<_>>()}),
                )
            }
            "setExceptionBreakpoints" => {
                let filters = args["filters"]
                    .as_array()
                    .ok_or("filters array is required")?;
                if filters.iter().any(|filter| filter != "all")
                    || args.get("filterOptions").is_some()
                    || args.get("exceptionOptions").is_some()
                {
                    return Err("only the unconditional all exception filter is supported".into());
                }
                self.active()?
                    .exception_breakpoints(!filters.is_empty())
                    .map_err(|error| error.to_string())?;
                Ok(json!({"breakpoints":[]}))
            }
            "configurationDone" => {
                self.active()?
                    .configuration_done()
                    .map_err(|e| e.to_string())?;
                Ok(json!({}))
            }
            "threads" => {
                self.active()?;
                Ok(json!({"threads":[{"id":1,"name":"Core safety producer"}]}))
            }
            "stackTrace" => {
                thread()?;
                let stack = self.active()?.stack().map_err(|e| e.to_string())?;
                let start = usize::try_from(unsigned(args, "startFrame", 0)?)
                    .map_err(|_| "startFrame overflow")?;
                let levels = usize::try_from(unsigned(args, "levels", 128)?.max(1))
                    .map_err(|_| "levels overflow")?;
                let levels = if args["levels"] == 0 { 128 } else { levels };
                if levels > 128 {
                    return Err("stack pagination bound".into());
                }
                Ok(
                    json!({"totalFrames":stack.len(),"stackFrames":stack.iter().skip(start).take(levels).collect::<Vec<_>>()}),
                )
            }
            "scopes" => Ok(
                json!({"scopes":self.active()?.scopes(id("frameId")?).map_err(|e|e.to_string())?}),
            ),
            "variables" => {
                let mut values = self
                    .active()?
                    .variables(id("variablesReference")?)
                    .map_err(|e| e.to_string())?;
                let start = usize::try_from(unsigned(args, "start", 0)?)
                    .map_err(|_| "variables start overflow")?;
                let count = usize::try_from(unsigned(args, "count", 4096)?)
                    .map_err(|_| "variables count overflow")?;
                let (start, count) = if self.capabilities.paging {
                    (start, if count == 0 { 4096 } else { count })
                } else {
                    (0, 4096)
                };
                if count > 4096 {
                    return Err("variables pagination bound".into());
                }
                match args.get("filter").and_then(Value::as_str) {
                    Some("indexed") => return Ok(json!({"variables":[]})),
                    Some("named") | None => {}
                    _ => return Err("invalid variables filter".into()),
                }
                if args.get("filter").is_some_and(|value| !value.is_string()) {
                    return Err("invalid variables filter".into());
                }
                if !self.capabilities.variable_types {
                    for value in &mut values {
                        if let Some(object) = value.as_object_mut() {
                            object.remove("type");
                        }
                    }
                }
                Ok(
                    json!({"variables":values.into_iter().skip(start).take(count).collect::<Vec<_>>()}),
                )
            }
            "source" => {
                let reference = args["sourceReference"]
                    .as_u64()
                    .or_else(|| args["source"]["sourceReference"].as_u64())
                    .ok_or("loaded sourceReference is required")?;
                Ok(
                    json!({"content":self.active()?.source(usize::try_from(reference).map_err(|_|"source reference overflow")?).map_err(|e|e.to_string())?,"mimeType":"text/x-tla"}),
                )
            }
            "evaluate" => {
                let expression = args["expression"]
                    .as_str()
                    .ok_or("expression is required")?;
                let frame = id("frameId")?;
                if ["source", "line", "column"]
                    .iter()
                    .any(|key| args.get(*key).is_some())
                {
                    return Err("bounded evaluation requires the stopped frame context only".into());
                }
                let mut result = self
                    .active()?
                    .evaluate(frame, expression)
                    .map_err(|error| error.to_string())?;
                if !self.capabilities.variable_types
                    && let Some(object) = result.as_object_mut()
                {
                    object.remove("type");
                }
                Ok(result)
            }
            "pause" => {
                thread()?;
                self.active()?.pause().map_err(|e| e.to_string())?;
                Ok(json!({}))
            }
            "continue" | "next" | "stepIn" | "stepOut" => {
                thread()?;
                let _single_thread = boolean(args, "singleThread", false)?;
                if args.get("targetId").is_some()
                    || args
                        .get("granularity")
                        .is_some_and(|value| value != "statement")
                {
                    return Err("only source-statement forward stepping is supported".into());
                }
                let session = self.active()?;
                match command {
                    "continue" => session.continue_check(),
                    "next" => session.next(),
                    "stepIn" => session.step_in(),
                    _ => session.step_out(),
                }
                .map_err(|e| e.to_string())?;
                self.last_exception = None;
                Ok(if command == "continue" {
                    json!({"allThreadsContinued":true})
                } else {
                    json!({})
                })
            }
            "exceptionInfo" => {
                thread()?;
                self.active()?.stack().map_err(|e| e.to_string())?;
                let description = self
                    .last_exception
                    .as_ref()
                    .ok_or("current stop is not an exception")?;
                Ok(
                    json!({"exceptionId":"dive.actual-producer","description":description,"breakMode":"always"}),
                )
            }
            "terminate" => {
                self.active()?.terminate();
                Ok(json!({}))
            }
            "disconnect" => {
                if !boolean(args, "terminateDebuggee", !self.attached_existing)? {
                    return Err("detaching a running checker is unsupported".into());
                }
                if let Some(session) = &self.session {
                    session.terminate();
                } else {
                    self.done = true;
                }
                Ok(json!({}))
            }
            _ => Err(format!("unsupported DAP request {command}")),
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "single framed request and asynchronous real producer event loop"
)]
fn serve<R: Read + Send + 'static, W: Write>(
    reader: R,
    writer: &mut W,
    request: CheckRequest,
    pending_check: Option<Session>,
) -> Result<u8, String> {
    let (send, receive) = mpsc::sync_channel(32);
    let _reader = std::thread::Builder::new()
        .name("dive-dap-input".into())
        .spawn(move || {
            let mut reader = BufReader::new(reader);
            loop {
                let result = frame(&mut reader);
                let done = !matches!(result, Ok(Some(_)));
                if send.send(result).is_err() || done {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    let mut wire = Wire {
        output: writer,
        sequence: 0,
    };
    let mut server = Server {
        capabilities: ClientCapabilities::default(),
        request,
        session: pending_check,
        initialized: false,
        attached: false,
        attached_existing: false,
        last_request: 0,
        last_exception: None,
        pending: None,
        done: false,
    };
    loop {
        match receive.recv_timeout(Duration::from_millis(5)) {
            Ok(Ok(Some(message))) => {
                let command = message["command"].as_str().unwrap_or("");
                let result = server.request(&message);
                let success = result.is_ok();
                wire.response(&message, result)?;
                if success && matches!(command, "launch" | "attach") {
                    wire.event("initialized", &json!({}))?;
                }
            }
            Ok(Ok(None)) => {
                if !server.attached && server.session.is_some() {
                    return Err("client closed before attaching to the pending check".into());
                }
                if let Some(session) = &server.session {
                    session.terminate();
                } else {
                    return Ok(0);
                }
            }
            Ok(Err(error)) => {
                if let Some(session) = &server.session {
                    session.terminate();
                }
                return Err(error);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if server.session.is_none() {
                    return Ok(0);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let event = if let Some(event) = server.pending.take() {
            Some(event)
        } else if server.attached {
            server
                .active()?
                .wait_event(Duration::ZERO)
                .map_err(|error| error.to_string())?
        } else {
            None
        };
        if let Some(event) = event {
            match event {
                Event::Prepared => {}
                Event::Stopped {
                    reason,
                    breakpoints,
                    description,
                    ..
                } => {
                    server.last_exception.clone_from(&description);
                    wire.event("stopped",&json!({"reason":reason,"threadId":1,"allThreadsStopped":true,"hitBreakpointIds":breakpoints,"description":description}))?;
                }
                Event::Completed(document) => {
                    let exit = u8::try_from(
                        document["exit_code"]
                            .as_u64()
                            .ok_or("checker completion has no native exit")?,
                    )
                    .map_err(|_| "checker exit overflow")?;
                    wire.event("dive/checkResult", &document)?;
                    wire.event("exited", &json!({"exitCode":exit}))?;
                    wire.event("terminated", &json!({}))?;
                    if let Some(session) = &mut server.session {
                        let _report = session.finish().map_err(|e| e.to_string())?;
                    }
                    return Ok(exit);
                }
                Event::Failed(message) => {
                    wire.event("output", &json!({"category":"stderr","output":message}))?;
                    wire.event("terminated", &json!({}))?;
                    return Ok(2);
                }
            }
        }
        if server.done {
            return Ok(0);
        }
    }
}

pub(crate) fn command(
    args: &[String],
    output: &mut impl Write,
    errors: &mut impl Write,
) -> ExitCode {
    let result = (|| -> Result<u8, String> {
        let mut request = CheckRequest::default();
        let mut listen = None;
        let mut stdio_requested = false;
        let mut index = 0;
        request.compat_tlc = match std::env::var("DIVE_COMPAT") {
            Ok(profile) if profile == "tlc" => true,
            Ok(profile) if profile == "dive" => false,
            Err(std::env::VarError::NotPresent) => false,
            _ => return Err("DIVE_COMPAT must be dive or tlc".into()),
        };
        while index < args.len() {
            let argument = &args[index];
            match argument.as_str() {
                "--stdio" => stdio_requested = true,
                "--compat=tlc" => request.compat_tlc = true,
                "--compat=dive" => request.compat_tlc = false,
                "--config" | "--listen" | "--seed" | "--fp" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| format!("{argument} needs a value"))?;
                    match argument.as_str() {
                        "--config" => request.config = Some(value.into()),
                        "--listen" => {
                            listen = Some(
                                value
                                    .parse::<SocketAddr>()
                                    .map_err(|_| "invalid debugger listen address")?,
                            );
                        }
                        "--seed" => {
                            request.seed = Some(dive_api::parse_seed(value).ok_or("invalid seed")?);
                        }
                        _ => {
                            request.fp = Some(
                                value
                                    .parse()
                                    .map_err(|_| "invalid fingerprint polynomial")?,
                            );
                        }
                    }
                }
                "-h" | "--help" => {
                    writeln!(errors,"USAGE: dive debug FILE.tla [--config FILE.cfg] [--compat=tlc] [--stdio | --listen 127.0.0.1:PORT] [--seed N] [--fp N]").map_err(|e|e.to_string())?;
                    return Ok(0);
                }
                _ if argument.starts_with('-') => {
                    return Err(format!("unsupported debugger option {argument}"));
                }
                _ => {
                    if !request.file.as_os_str().is_empty() {
                        return Err("exactly one debugger program is required".into());
                    }
                    request.file = argument.into();
                }
            }
            index += 1;
        }
        if request.file.as_os_str().is_empty() {
            return Err("debugger requires an explicit CLI-authorized program".into());
        }
        if stdio_requested && listen.is_some() {
            return Err("--stdio and --listen are mutually exclusive".into());
        }
        if let Some(address) = listen {
            if !address.ip().is_loopback() {
                return Err("debugger listener requires a loopback address".into());
            }
            let listener = TcpListener::bind(address).map_err(|e| e.to_string())?;
            writeln!(
                errors,
                "Debugger listening on {}",
                listener.local_addr().map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            errors.flush().map_err(|e| e.to_string())?;
            let pending =
                Session::launch(request.clone(), false).map_err(|error| error.to_string())?;
            let (stream, peer) = listener.accept().map_err(|e| e.to_string())?;
            if !peer.ip().is_loopback() {
                return Err("non-loopback debugger peer refused".into());
            }
            let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
            serve(stream, &mut writer, request, Some(pending))
        } else {
            serve(std::io::stdin(), output, request, None)
        }
    })();
    match result {
        Ok(exit) => ExitCode::from(exit),
        Err(error) => {
            let _ = writeln!(errors, "DIVE-D0001: {error}");
            ExitCode::from(2)
        }
    }
}
