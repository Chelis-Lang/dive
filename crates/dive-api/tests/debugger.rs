//! The session controls an actual compiled checker, not a replay evaluator.
use dive_api::{
    CheckRequest, Engine, check_file_engine,
    debug::{Event, Session},
};
use std::time::Duration;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one real stopped/stepped/completed workflow with semantic and handle-negative controls in both profiles"
)]
fn helper_breakpoint_inspects_real_storage_steps_and_finishes_the_existing_core_check()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("dive-debugger-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("DebugCheck.tla");
    std::fs::write(
        &file,
        "---- MODULE DebugCheck ----\nEXTENDS Naturals\nVARIABLE x,y\n\tAdd(a,b) == (* 🙂 *) a+b\nIdentity(v) == v\nUnused == 1+2\nInit == /\\ x=0 /\\ y=0\nNext == /\\ x<2 /\\ x'=Add(x,1) /\\ y'=Identity(y)+2\nInv == y=2*x\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )?;
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            file: file.clone(),
            compat_tlc,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let plain = check_file_engine(&request, Engine::Core)?.to_json();
        assert_eq!(plain["verdict"], "success", "{plain}");
        let mut session = Session::launch(request, false)?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Prepared)
        ));
        let breakpoints = session.set_breakpoints(&file.to_string_lossy(), &[4, 5, 6])?;
        assert_eq!(breakpoints.len(), 3);
        assert!(!breakpoints[1].verified);
        assert!(!breakpoints[2].verified);
        assert!(breakpoints[0].verified);
        session.configuration_done()?;
        let stopped = session
            .wait_event(Duration::from_secs(10))?
            .ok_or("breakpoint event")?;
        assert!(
            matches!(stopped,Event::Stopped {ref reason,..} if reason=="breakpoint"),
            "{stopped:?}"
        );
        let stack = session.stack()?;
        assert_eq!(stack[0]["name"], "Add");
        let source = std::fs::read_to_string(&file)?;
        let helper = source.lines().nth(3).ok_or("helper line")?;
        let before = helper.split_once("a+b").ok_or("helper expression")?.0;
        assert_eq!(
            stack[0]["column"],
            serde_json::json!(before.encode_utf16().count() + 1)
        );
        assert!(
            stack.iter().any(|frame| frame["name"] == "Next"),
            "{stack:?}"
        );
        let frame = stack[0]["id"].as_u64().ok_or("frame")?;
        let scopes = session.scopes(frame)?;
        let locals_reference = scopes[2]["variablesReference"].as_u64().ok_or("locals")?;
        let locals = session.variables(locals_reference)?;
        assert!(
            locals
                .iter()
                .any(|local| local["name"] == "a" && local["value"] == "0"),
            "{locals:?}"
        );
        assert!(
            locals
                .iter()
                .any(|local| local["name"] == "b" && local["value"] == "1"),
            "{locals:?}"
        );
        assert_eq!(session.evaluate(frame, "a+b")?["result"], "1");
        assert!(session.evaluate(frame, "INSTANCE Secret").is_err());
        assert!(session.evaluate(frame, "PrintT(a)").is_err());
        assert!(session.evaluate(frame, "1 \\div 0").is_err());
        assert!(
            session
                .set_breakpoints("unloaded-private-source.tla", &[1])
                .is_err()
        );
        session.set_breakpoints(&file.to_string_lossy(), &[])?;
        session.step_in()?;
        let step_event = session
            .wait_event(Duration::from_secs(10))?
            .ok_or("step event")?;
        assert!(
            matches!(step_event,Event::Stopped {ref reason,..} if reason=="step"),
            "{step_event:?}"
        );
        assert!(session.evaluate(frame, "a").is_err());
        assert!(session.scopes(frame).is_err());
        assert!(session.variables(locals_reference).is_err());
        session.continue_check()?;
        let completed = session
            .wait_event(Duration::from_secs(10))?
            .ok_or("completion event")?;
        assert!(matches!(completed, Event::Completed(_)), "{completed:?}");
        let observed = session.finish()?.to_json();
        assert_eq!(observed["verdict"], "success");
        for field in [
            "verdict",
            "statistics",
            "tlc",
            "output",
            "counterexample",
            "warnings",
        ] {
            assert_eq!(observed[field], plain[field], "{field}");
        }
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn instance_helper_source_and_unforced_locals_keep_actual_provenance_and_demand()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("dive-debugger-instance-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("Outer.tla");
    let helper = directory.join("Inner.tla");
    std::fs::write(
        &helper,
        "---- MODULE Inner ----\nEXTENDS Naturals,TLC\nVARIABLE x\nBump(a,unused) == a+1\nGo == x'=Bump(x,PrintT(\"unused\"))\n====\n",
    )?;
    std::fs::write(
        &file,
        "---- MODULE Outer ----\nEXTENDS Naturals,TLC\nVARIABLE x,y\nI == INSTANCE Inner WITH x<-x\nInit == /\\ x=0 /\\ y=0\nNext == /\\ x<2 /\\ I!Go /\\ y'=y\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    )?;
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            file: file.clone(),
            compat_tlc,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..Default::default()
        };
        let plain = check_file_engine(&request, Engine::Core)?.to_json();
        assert_eq!(plain["verdict"], "success", "{plain}");
        let mut session = Session::launch(request, false)?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Prepared)
        ));
        assert!(session.set_breakpoints(&helper.to_string_lossy(), &[4])?[0].verified);
        session.configuration_done()?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Stopped { .. })
        ));
        let stack = session.stack()?;
        let top = &stack[0];
        assert!(
            top["name"]
                .as_str()
                .is_some_and(|name| name.contains("Bump")),
            "{stack:?}"
        );
        assert_eq!(top["source"]["path"], helper.to_string_lossy().as_ref());
        assert!(
            top["substitutions"]
                .as_array()
                .is_some_and(
                    |entries| entries.iter().any(|entry| entry["declaration"] == "x"
                        && entry["actual"]["path"] == file.to_string_lossy().as_ref())
                ),
            "{top}"
        );
        let frame = top["id"].as_u64().ok_or("frame")?;
        let scopes = session.scopes(frame)?;
        let locals =
            session.variables(scopes[2]["variablesReference"].as_u64().ok_or("locals")?)?;
        assert!(
            locals
                .iter()
                .any(|local| local["name"] == "a" && local["value"] == "0"),
            "{locals:?}"
        );
        assert!(
            locals
                .iter()
                .any(|local| local["name"] == "unused" && local["scalar"].is_null()),
            "{locals:?}"
        );
        assert!(session.evaluate(frame, "unused").is_err());
        session.set_breakpoints(&helper.to_string_lossy(), &[])?;
        session.continue_check()?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Completed(_))
        ));
        let observed = session.finish()?.to_json();
        assert_eq!(observed["output"], serde_json::json!([]));
        for field in [
            "verdict",
            "statistics",
            "tlc",
            "output",
            "counterexample",
            "warnings",
        ] {
            assert_eq!(observed[field], plain[field], "{field}");
        }
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn nested_inlined_helpers_have_distinct_real_activations_without_rehitting_each_operand()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("dive-debug-nested-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("Nested.tla");
    std::fs::write(
        &file,
        "---- MODULE Nested ----\nEXTENDS Naturals\nVARIABLE x,y\nPlus(a,b) == a+b\nInit == /\\ x=0 /\\ y=0\nNext == /\\ x<2 /\\ x'=Plus(Plus(x,1),1) /\\ y'=y\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
    )?;
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            file: file.clone(),
            compat_tlc,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..Default::default()
        };
        let plain = check_file_engine(&request, Engine::Core)?.to_json();
        let mut session = Session::launch(request, false)?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Prepared)
        ));
        assert!(session.set_breakpoints(&file.to_string_lossy(), &[4])?[0].verified);
        session.configuration_done()?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Stopped { .. })
        ));
        let first = session.stack()?;
        let outer = first[0]["environment"].clone();
        session.continue_check()?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Stopped { .. })
        ));
        let nested = session.stack()?;
        assert_eq!(nested[0]["name"], "Plus");
        assert_ne!(nested[0]["environment"], outer);
        assert!(
            nested
                .iter()
                .filter(|frame| frame["name"] == "Plus")
                .count()
                >= 2,
            "{nested:?}"
        );
        session.set_breakpoints(&file.to_string_lossy(), &[])?;
        session.continue_check()?;
        assert!(matches!(
            session.wait_event(Duration::from_secs(10))?,
            Some(Event::Completed(_))
        ));
        let observed = session.finish()?.to_json();
        for field in [
            "verdict",
            "statistics",
            "tlc",
            "output",
            "counterexample",
            "warnings",
        ] {
            assert_eq!(observed[field], plain[field], "{field}");
        }
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn implicitly_selected_temporal_check_is_refused_before_observer_preparation()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("dive-debug-temporal-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("Temporal.tla");
    std::fs::write(
        &file,
        "---- MODULE Temporal ----\nVARIABLE x\nInit == x=0\nNext == x'=x\nLive == []<>(x=0)\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nPROPERTY Live\n",
    )?;
    for compat_tlc in [false, true] {
        let session = Session::launch(
            CheckRequest {
                file: file.clone(),
                compat_tlc,
                seed: Some(7),
                fp: Some(0),
                ..Default::default()
            },
            false,
        )?;
        let Some(Event::Completed(report)) = session.wait_event(Duration::from_secs(10))? else {
            return Err("temporal refusal must precede Prepared".into());
        };
        assert_eq!(report["verdict"], "unsupported", "{report}");
        assert!(
            report
                .to_string()
                .contains("compiled debugger with temporal checking")
        );
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
