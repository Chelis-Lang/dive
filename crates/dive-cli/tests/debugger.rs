//! Real framed DAP clients drive the existing Core checker.
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn read(reader: &mut impl BufRead) -> Result<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err("DAP EOF".into());
        }
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = Some(value.trim().parse::<usize>()?);
        }
    }
    let length = length.ok_or("Content-Length")?;
    if length > 8 << 20 {
        return Err("frame bound".into());
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
struct Client {
    child: Child,
    input: Box<dyn Write + Send>,
    messages: Receiver<std::result::Result<Value, String>>,
    pending: Vec<Value>,
    sequence: u64,
}
impl Client {
    fn new(
        mut child: Child,
        input: Option<Box<dyn Write + Send>>,
        reader: Option<Box<dyn Read + Send>>,
    ) -> Result<Self> {
        let input = if let Some(input) = input {
            input
        } else {
            Box::new(child.stdin.take().ok_or("stdin")?) as Box<dyn Write + Send>
        };
        let reader = if let Some(reader) = reader {
            reader
        } else {
            Box::new(child.stdout.take().ok_or("stdout")?) as Box<dyn Read + Send>
        };
        let (send, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader);
            loop {
                let value = read(&mut reader).map_err(|error| error.to_string());
                let done = value.is_err();
                if send.send(value).is_err() || done {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            messages,
            pending: Vec::new(),
            sequence: 0,
        })
    }
    fn launch(file: &std::path::Path, compat: bool) -> Result<Self> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dive"));
        command
            .arg("debug")
            .arg(file)
            .args(["--stdio", "--seed", "7", "--fp", "0"]);
        if compat {
            command.arg("--compat=tlc");
        }
        Self::new(
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
            None,
            None,
        )
    }
    fn message(&mut self) -> Result<Value> {
        self.messages
            .recv_timeout(Duration::from_secs(10))?
            .map_err(Into::into)
    }
    fn request(&mut self, command: &str, args: Value) -> Result<Value> {
        self.sequence += 1;
        let sequence = self.sequence;
        let mut message = json!({"seq":sequence,"type":"request","command":command});
        message["arguments"] = args;
        let bytes = serde_json::to_vec(&message)?;
        write!(self.input, "Content-Length: {}\r\n\r\n", bytes.len())?;
        self.input.write_all(&bytes)?;
        self.input.flush()?;
        loop {
            let message = self.message()?;
            if message["type"] == "response" && message["request_seq"] == sequence {
                return Ok(message);
            }
            self.pending.push(message);
        }
    }
    fn ok(&mut self, command: &str, args: Value) -> Result<Value> {
        let response = self.request(command, args)?;
        assert_eq!(response["success"], true, "{response}");
        Ok(response["body"].clone())
    }
    fn event(&mut self, name: &str) -> Result<Value> {
        if let Some(index) = self.pending.iter().position(|event| event["event"] == name) {
            return Ok(self.pending.remove(index)["body"].clone());
        }
        loop {
            let event = self.message()?;
            if event["event"] == name {
                return Ok(event["body"].clone());
            }
            self.pending.push(event);
        }
    }
    fn initialize(&mut self, operation: &str) -> Result<()> {
        self.ok(
            "initialize",
            json!({"adapterID":"dive","supportsVariableType":true,"supportsVariablePaging":true,"linesStartAt1":true,"columnsStartAt1":true,"pathFormat":"path"}),
        )?;
        self.ok(operation, json!({}))?;
        self.event("initialized")?;
        Ok(())
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn model(name: &str, invariant: &str) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let dir = std::env::temp_dir().join(format!("dive-dap-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("{name}.tla"));
    std::fs::write(
        &file,
        format!(
            "---- MODULE {name} ----\nEXTENDS Naturals\nVARIABLE x,y\nAdd(a,b) == a+b\nInit == /\\ x=0 /\\ y=0\nNext == /\\ x<2 /\\ x'=Add(x,1) /\\ y'=Add(y,2)\nInv == {invariant}\n====\n"
        ),
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )?;
    Ok((dir, file))
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one real framed conversation covers execution, inspection and stale/malformed request controls"
)]
fn framed_launch_breakpoint_locals_evaluation_all_forward_steps_and_real_completion() -> Result<()>
{
    let (dir, file) = model("DapCheck", "y=2*x")?;
    for compat in [false, true] {
        let mut client = Client::launch(&file, compat)?;
        client.initialize("launch")?;
        let bps = client.ok(
            "setBreakpoints",
            json!({"source":{"path":file},"breakpoints":[{"line":4}]}),
        )?;
        assert_eq!(bps["breakpoints"][0]["verified"], true);
        assert_eq!(
            client.request(
                "setBreakpoints",
                json!({"source":{"path":file},"sourceModified":true,"breakpoints":[]})
            )?["success"],
            false
        );
        assert_eq!(
            client.request(
                "setBreakpoints",
                json!({"source":{"path":"private-unloaded.tla"},"breakpoints":[{"line":1}]})
            )?["success"],
            false
        );
        client.ok("configurationDone", json!({}))?;
        assert_eq!(client.event("stopped")?["reason"], "breakpoint");
        let stack = client.ok("stackTrace", json!({"threadId":1}))?;
        let frame = stack["stackFrames"][0]["id"].as_u64().ok_or("frame")?;
        assert_eq!(stack["stackFrames"][0]["name"], "Add");
        let scopes = client.ok("scopes", json!({"frameId":frame}))?;
        let reference = scopes["scopes"][2]["variablesReference"]
            .as_u64()
            .ok_or("locals")?;
        let variables = client.ok("variables", json!({"variablesReference":reference}))?;
        assert!(
            variables["variables"]
                .as_array()
                .ok_or("variables")?
                .iter()
                .any(|value| value["name"] == "a" && value["value"] == "0")
        );
        assert_eq!(
            client.ok("evaluate", json!({"frameId":frame,"expression":"a+b"}))?["result"],
            "1"
        );
        assert_eq!(
            client.request("stackTrace", json!({"threadId":1,"startFrame":-1}))?["success"],
            false
        );
        assert_eq!(
            client.request("evaluate", json!({"frameId":"stale","expression":"a"}))?["success"],
            false
        );
        assert_eq!(
            client.request("next", json!({"threadId":1,"granularity":"instruction"}))?["success"],
            false
        );
        assert_eq!(
            client.ok(
                "variables",
                json!({"variablesReference":reference,"count":0})
            )?["variables"],
            variables["variables"]
        );
        assert_eq!(
            client.ok(
                "variables",
                json!({"variablesReference":reference,"filter":"indexed"})
            )?["variables"],
            json!([])
        );
        assert_eq!(
            client.ok(
                "variables",
                json!({"variablesReference":reference,"start":9999})
            )?["variables"],
            json!([])
        );

        assert_eq!(
            client.request(
                "evaluate",
                json!({"frameId":frame,"expression":"INSTANCE Private"})
            )?["success"],
            false
        );
        let source = stack["stackFrames"][0]["source"]["sourceReference"]
            .as_u64()
            .ok_or("source")?;
        assert!(
            client.ok("source", json!({"sourceReference":source}))?["content"]
                .as_str()
                .is_some_and(|source| source.contains("Add(a,b)"))
        );
        client.ok(
            "setBreakpoints",
            json!({"source":{"path":file},"breakpoints":[]}),
        )?;
        for operation in ["next", "stepIn", "stepOut"] {
            client.ok(operation, json!({"threadId":1}))?;
            assert_eq!(client.event("stopped")?["reason"], "step");
        }
        assert_eq!(
            client.request("variables", json!({"variablesReference":reference}))?["success"],
            false
        );
        client.ok("continue", json!({"threadId":1}))?;
        let report = client.event("dive/checkResult")?;
        assert_eq!(report["verdict"], "success");
        assert_eq!(report["statistics"]["distinct"], 3);
        assert_eq!(report["statistics"]["generated"], 3);
        client.event("terminated")?;
        assert_eq!(client.child.wait()?.code(), Some(0));
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
#[test]
fn framed_exception_stop_preserves_the_ordinary_independently_validated_violation() -> Result<()> {
    let (dir, file) = model("DapViolation", "y=0")?;
    for compat in [false, true] {
        let mut client = Client::launch(&file, compat)?;
        client.initialize("launch")?;
        client.ok("setExceptionBreakpoints", json!({"filters":["all"]}))?;
        client.ok("configurationDone", json!({}))?;
        let stopped = client.event("stopped")?;
        assert_eq!(stopped["reason"], "exception");
        assert!(
            client.ok("exceptionInfo", json!({"threadId":1}))?["description"]
                .as_str()
                .is_some_and(|text| text.contains("Inv"))
        );
        client.ok("continue", json!({"threadId":1}))?;
        let report = client.event("dive/checkResult")?;
        assert_eq!(report["verdict"], "invariant-violation");
        let plain = dive_api::check_file_engine(
            &dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc: compat,
                workers: Some(1),
                seed: Some(7),
                fp: Some(0),
                ..Default::default()
            },
            dive_api::Engine::Core,
        )?
        .to_json_with_exit_convention(dive_api::ExitConvention::Native);
        for field in [
            "verdict",
            "statistics",
            "property",
            "tlc",
            "output",
            "counterexample",
            "warnings",
            "exit_code",
        ] {
            assert_eq!(report[field], plain[field], "{field}");
        }

        let spec = dive_api::trace::SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &dive_api::trace::TraceOptions {
                compat_tlc: compat,
                run: Some((7, 0)),
                ..Default::default()
            },
        )?;
        let word = dive_api::trace::TraceDoc::from_json(&report)?;
        assert_eq!(
            spec.try_validate(&word)?.validation,
            dive_api::trace::Validation::Valid
        );
        client.event("terminated")?;
        assert_eq!(client.child.wait()?.code(), Some(10));
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn real_loopback_attach_stops_the_pending_check_and_termination_cannot_report_success() -> Result<()>
{
    let (dir, file) = model("DapAttach", "y=2*x")?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_dive"))
        .arg("debug")
        .arg(&file)
        .args(["--listen", "127.0.0.1:0", "--seed", "7", "--fp", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().ok_or("listener stderr")?;
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stderr)
            .read_line(&mut line)
            .map(|_| line)
            .map_err(|error| error.to_string());
        let _ = send.send(result);
    });
    let line = receive.recv_timeout(Duration::from_secs(10))??;
    let address = line
        .trim()
        .strip_prefix("Debugger listening on ")
        .ok_or("actual listener address")?;
    let stream = std::net::TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let writer = stream.try_clone()?;
    let mut client = Client::new(child, Some(Box::new(writer)), Some(Box::new(stream)))?;
    client.ok("initialize", json!({"adapterID":"dive"}))?;
    client.ok("attach", json!({"stopOnEntry":true}))?;
    client.event("initialized")?;
    client.ok("configurationDone", json!({}))?;
    assert_eq!(client.event("stopped")?["reason"], "entry");
    client.ok("terminate", json!({}))?;
    let report = client.event("dive/checkResult")?;
    assert_ne!(report["verdict"], "success");
    assert!(report.to_string().contains("terminated"));
    client.event("terminated")?;
    assert_eq!(client.child.wait()?.code(), Some(3));
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn pause_suspends_real_running_search_and_invalid_frames_fail_before_model_loading() -> Result<()> {
    let (dir, file) = model("DapPause", "TRUE")?;
    let source = std::fs::read_to_string(&file)?.replace("x<2", "TRUE");
    std::fs::write(&file, source)?;
    let mut client = Client::launch(&file, false)?;
    client.initialize("launch")?;
    client.ok("configurationDone", json!({}))?;
    client.ok("pause", json!({"threadId":1}))?;
    assert_eq!(client.event("stopped")?["reason"], "pause");
    assert_ne!(
        client.ok("stackTrace", json!({"threadId":1}))?["stackFrames"]
            .as_array()
            .ok_or("stack")?
            .as_slice(),
        [] as [Value; 0]
    );
    client.ok("terminate", json!({}))?;
    assert_ne!(client.event("dive/checkResult")?["verdict"], "success");
    client.event("terminated")?;
    assert_eq!(client.child.wait()?.code(), Some(3));
    let mut bad = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["debug", "missing-not-loaded.tla"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    bad.stdin
        .take()
        .ok_or("bad stdin")?
        .write_all(b"Content-Length: 999999999\r\n\r\n")?;
    let output = bad.wait_with_output()?;
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, [] as [u8; 0]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("frame byte bound"));
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
