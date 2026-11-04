//! Bounded real socket faults against actual native worker evaluation.
#![allow(
    clippy::expect_used,
    reason = "private authenticated transport regression controls"
)]
use super::*;
use std::net::Shutdown;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
struct KeyFile(PathBuf);
impl KeyFile {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-worker-fault-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, [61; 32]).expect("test key file");
        Self(path)
    }
}
impl Drop for KeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn pair() -> (Channel, Channel) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test listener");
    let a = TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
    let (b, _) = listener.accept().expect("accept");
    let binding = Binding {
        identity: [1; 32],
        run: [2; 32],
        client: [3; 32],
        server: [4; 32],
        ticket: Ticket {
            epoch: 1,
            work: 2,
            attempt: 0,
        },
    };
    let channel = |stream, coordinator| Channel {
        stream,
        binding,
        send: 0,
        receive: 0,
        coordinator,
        deadline: Instant::now() + Duration::from_secs(2),
    };
    (channel(a, true), channel(b, false))
}
#[test]
fn bounded_frames_reject_corruption_stale_binding_reflection_future_and_oversize() {
    let key = Secret([9; 32]);
    let usage = Mutex::new(Usage::default());
    let (mut sender, mut receiver) = pair();
    sender.write(&key, WORK, b"one").expect("first frame");
    sender.send = 0;
    sender.write(&key, WORK, b"one").expect("duplicate frame");
    sender.write(&key, WORK, b"two").expect("next frame");
    assert_eq!(
        receiver.read(&key, &usage).expect("first"),
        (WORK, b"one".to_vec())
    );
    assert_eq!(
        receiver.read(&key, &usage).expect("skip duplicate"),
        (WORK, b"two".to_vec())
    );
    assert_eq!(usage.lock().expect("usage").duplicate_frames, 1);
    for mutation in 0..6 {
        let (mut sender, mut receiver) = pair();
        match mutation {
            0 => sender.binding.ticket.epoch += 1,
            1 => sender.binding.ticket.attempt += 1,
            2 => sender.binding.run[0] ^= 1,
            3 => sender.binding.server[0] ^= 1,
            4 => sender.coordinator = false,
            _ => sender.send = 1,
        }
        sender
            .write(&key, WORK, b"bound payload")
            .expect("altered binding");
        assert!(receiver.read(&key, &usage).is_err(), "mutation {mutation}");
    }
    let (mut sender, mut receiver) = pair();
    let mut header = [0; 17];
    header[0] = WORK;
    header[9..].copy_from_slice(&((MAX_FRAME_BYTES + 1) as u64).to_le_bytes());
    sender.stream.write_all(&header).expect("oversized header");
    assert!(
        receiver
            .read(&key, &usage)
            .expect_err("admission")
            .message()
            .contains("frame byte admission")
    );
    let (mut sender, mut receiver) = pair();
    let mut header = [0; 17];
    header[0] = WORK;
    header[9..].copy_from_slice(&3u64.to_le_bytes());
    sender.stream.write_all(&header).expect("header");
    sender.stream.write_all(b"bad").expect("payload");
    sender.stream.write_all(&[0; 32]).expect("invalid tag");
    assert!(
        receiver
            .read(&key, &usage)
            .expect_err("MAC")
            .message()
            .contains("authentication failed")
    );
    let (sender, mut receiver) = pair();
    sender.stream.shutdown(Shutdown::Write).expect("truncation");
    assert!(
        receiver
            .read(&key, &usage)
            .expect_err("truncation")
            .message()
            .starts_with("DIVE-S0502:")
    );
    let (mut sender, mut receiver) = pair();
    receiver.deadline = Instant::now() + Duration::from_millis(40);
    let mut header = [0; 17];
    header[0] = WORK;
    header[9..].copy_from_slice(&3u64.to_le_bytes());
    let tag = key
        .tag(&[
            Channel::direction(true),
            &sender.binding.context(),
            &header,
            b"one",
        ])
        .expect("valid trickle tag");
    let mut frame = header.to_vec();
    frame.extend_from_slice(b"one");
    frame.extend_from_slice(&tag);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            for chunk in frame.chunks(2) {
                if sender.stream.write_all(chunk).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(3));
            }
        });
        assert!(
            receiver.read(&key, &usage).is_err(),
            "a valid trickled frame cannot extend the absolute deadline"
        );
    });
}

#[derive(Clone, Copy)]
enum Fault {
    DropPartial,
    DropNoRetry,
    LaterDrop,
    Duplicate,
    Corrupt,
    Stale,
}
struct Injection {
    mode: Fault,
    fired: AtomicBool,
    prior: Mutex<Option<Vec<u8>>>,
}

// Copies framed TCP bytes without knowing the key or evaluating Next.
fn frames(mut from: TcpStream, mut to: TcpStream, injection: Option<&Injection>) {
    loop {
        let mut header = [0; 17];
        if from.read_exact(&mut header).is_err() {
            break;
        }
        let len = usize::try_from(u64::from_le_bytes(
            header[9..].try_into().expect("frame length"),
        ))
        .expect("bounded native word");
        if len > MAX_FRAME_BYTES {
            break;
        }
        let mut bytes = header.to_vec();
        bytes.resize(17 + len + 32, 0);
        if from.read_exact(&mut bytes[17..]).is_err() {
            break;
        }
        if let Some(injection) = injection {
            let kind = header[0];
            match injection.mode {
                Fault::DropPartial | Fault::DropNoRetry | Fault::LaterDrop
                    if kind == RESULT && !injection.fired.swap(true, Ordering::Relaxed) =>
                {
                    let _ = to.write_all(&bytes[..17 + len / 2]);
                    break;
                }
                Fault::Corrupt
                    if kind == RESULT && !injection.fired.swap(true, Ordering::Relaxed) =>
                {
                    bytes[17] ^= 1;
                }
                Fault::Duplicate
                    if kind == QUERY && !injection.fired.swap(true, Ordering::Relaxed) =>
                {
                    if to.write_all(&bytes).is_err() {
                        break;
                    }
                }
                Fault::Stale if kind == RESULT => {
                    let mut prior = injection.prior.lock().expect("prior frame");
                    if let Some(prior) = prior.as_ref() {
                        if !injection.fired.swap(true, Ordering::Relaxed) {
                            bytes = prior.clone();
                        }
                    } else {
                        *prior = Some(bytes.clone());
                    }
                }
                _ => {}
            }
        }
        if to.write_all(&bytes).is_err() {
            break;
        }
    }
    let _ = from.shutdown(Shutdown::Both);
    let _ = to.shutdown(Shutdown::Both);
}
fn proxy(listener: &TcpListener, target: SocketAddr, stop: &AtomicBool, injection: &Injection) {
    listener.set_nonblocking(true).expect("nonblocking proxy");
    std::thread::scope(|scope| {
        while !stop.load(Ordering::Relaxed) {
            let (mut incoming, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(_) => break,
            };
            scope.spawn(move || {
                let mut outgoing = TcpStream::connect(target).expect("actual worker");
                timeouts(&incoming, Duration::from_secs(2)).expect("proxy timeout");
                timeouts(&outgoing, Duration::from_secs(2)).expect("worker timeout");
                let mut hello = [0; 160];
                if incoming
                    .read_exact(&mut hello)
                    .and_then(|()| outgoing.write_all(&hello))
                    .is_err()
                {
                    return;
                }
                let mut ack = [0; 101];
                if outgoing
                    .read_exact(&mut ack)
                    .and_then(|()| incoming.write_all(&ack))
                    .is_err()
                {
                    return;
                }
                let reverse_in = outgoing.try_clone().expect("read worker");
                let reverse_out = incoming.try_clone().expect("write coordinator");
                std::thread::scope(|scope| {
                    scope.spawn(move || frames(reverse_in, reverse_out, Some(injection)));
                    frames(incoming, outgoing, None);
                });
            });
        }
    });
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "actual worker, transport mutation and completion controls share one bounded lifecycle"
)]
fn actual_retry_and_duplicate_match_complete_search_and_corrupt_stale_never_hold() {
    let key = KeyFile::new();
    for tlc in [false, true] {
        for exact in [false, true] {
            for mode in [
                Fault::DropPartial,
                Fault::DropNoRetry,
                Fault::LaterDrop,
                Fault::Duplicate,
                Fault::Corrupt,
                Fault::Stale,
            ] {
                let semantics = if tlc {
                    dive_core::Semantics::tlc()
                } else {
                    dive_core::Semantics::default()
                };
                let store = Arc::new(Store::with_semantics(true, semantics));
                let plan = fault_plan(&store, mode);
                let config = || crate::Config {
                    opts: Opts::default(),
                    max_states: None,
                    max_depth: None,
                    workers: 1,
                    warn: true,
                    sample: None,
                };
                let storage = crate::storage::Storage {
                    exact,
                    ..crate::storage::Storage::default()
                };
                let local = crate::explore_with_storage(
                    &plan,
                    &store,
                    config(),
                    &|| false,
                    None,
                    None,
                    &storage,
                );
                let mut workers = Vec::new();
                let mut listeners = Vec::new();
                let mut addresses = Vec::new();
                for _ in 0..2 {
                    let listener = TcpListener::bind("127.0.0.1:0").expect("worker listener");
                    addresses.push(listener.local_addr().expect("address"));
                    listeners.push(listener);
                    let owner = Arc::new(Store::with_semantics(true, semantics));
                    let plan = fault_plan(&owner, mode);
                    workers.push(
                        Worker::new(
                            plan,
                            owner,
                            Opts::default(),
                            b"fixed fault model",
                            &key.0,
                            Duration::from_secs(3),
                        )
                        .expect("worker"),
                    );
                }
                let listener = TcpListener::bind("127.0.0.1:0").expect("proxy listener");
                let address = listener.local_addr().expect("proxy address");
                let stop = Arc::new(AtomicBool::new(false));
                let injection = Injection {
                    mode,
                    fired: AtomicBool::new(false),
                    prior: Mutex::new(None),
                };
                std::thread::scope(|scope| {
                    let _guard = Stop(Arc::clone(&stop));
                    for (worker, listener) in workers.iter().zip(&listeners) {
                        let stop = &stop;
                        scope.spawn(move || {
                            worker
                                .serve(listener, None, &|| stop.load(Ordering::Relaxed))
                                .expect("serve")
                        });
                    }
                    let stop = &stop;
                    let injection = &injection;
                    let listener = &listener;
                    let later = matches!(mode, Fault::LaterDrop);
                    let target = addresses[usize::from(later)];
                    scope.spawn(move || proxy(listener, target, stop, injection));
                    let mut options = Options::new(
                        if later {
                            vec![addresses[0], address]
                        } else {
                            vec![address, addresses[1]]
                        },
                        key.0.clone(),
                    );
                    if matches!(mode, Fault::DropNoRetry | Fault::LaterDrop) {
                        options.retries = 0;
                    }
                    let coordinator = Coordinator::new(options, b"fixed fault model", &store)
                        .expect("coordinator");
                    let out = crate::explore_distributed(
                        &plan,
                        &store,
                        config(),
                        &|| false,
                        None,
                        &storage,
                        &coordinator,
                    );
                    assert!(
                        injection.fired.load(Ordering::Relaxed),
                        "fault was actually delivered"
                    );
                    let facts = coordinator.usage();
                    match mode {
                        Fault::DropPartial | Fault::Duplicate => {
                            assert_eq!(out.verdict, local.verdict, "{:?}", out.verdict);
                            assert_eq!(out.stats, local.stats);
                            assert_eq!(facts.assigned_units, 128);
                            assert_eq!(facts.completed_units, 128);
                            assert_eq!(facts.failures, [] as [String; 0]);
                            if matches!(mode, Fault::DropPartial) {
                                assert_eq!(facts.retries, 1);
                            } else {
                                assert_eq!(facts.duplicate_frames, 1);
                                assert_eq!(facts.retries, 0);
                            }
                        }
                        Fault::Corrupt | Fault::Stale | Fault::DropNoRetry => {
                            assert!(!matches!(out.verdict, crate::Verdict::Success));
                            assert_ne!(facts.failures, [] as [String; 0]);
                            assert!(out.trace.is_none());
                            assert_eq!(facts.retries, 0);
                        }
                        Fault::LaterDrop => {
                            assert_eq!(out.verdict, local.verdict);
                            assert!(matches!(
                                out.verdict,
                                crate::Verdict::InvariantViolated { .. }
                            ));
                            assert_eq!(out.stats, local.stats);
                            assert_eq!(out.trace, local.trace);
                            assert_eq!(facts.completed_units, 1);
                            assert_eq!(facts.failed_units, 1);
                            assert_eq!(facts.failures, [] as [String; 0]);
                        }
                    }
                });
            }
        }
    }
}

fn fault_plan(store: &Arc<Store>, mode: Fault) -> Plan {
    let mut plan = crate::tests::frontier_plan(store, false).expect("independent plan");
    if matches!(mode, Fault::LaterDrop) {
        use dive_core::plan::{Arith, Check, Cmp, Conjunct, GNode, Node};
        plan.code.nodes.extend([
            Node::Const(Handle::int(128).expect("bound")),
            Node::Arith(Arith::Add, 1, 3),
            Node::Cmp(Cmp::Lt, 1, 3),
        ]);
        plan.code.gnodes[1] = GNode::Assign { var: 0, rhs: 4 };
        plan.invariants.push(Check {
            name: "Inv".into(),
            frame: 0,
            conjuncts: vec![Conjunct {
                p: 5,
                reads: 1,
                skippable: false,
                site: None,
            }],
        });
    }
    plan
}

#[test]
fn cancellation_after_membership_never_certifies_a_partial_source() {
    let key = KeyFile::new();
    let store = Arc::new(Store::new(true));
    let plan = crate::tests::frontier_plan(&store, false).expect("plan");
    let mut workers = Vec::new();
    let mut listeners = Vec::new();
    let mut addresses = Vec::new();
    for _ in 0..2 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        addresses.push(listener.local_addr().expect("address"));
        listeners.push(listener);
        let store = Arc::new(Store::new(true));
        let plan = crate::tests::frontier_plan(&store, false).expect("independent plan");
        workers.push(
            Worker::new(
                plan,
                store,
                Opts::default(),
                b"cancel model",
                &key.0,
                Duration::from_secs(2),
            )
            .expect("worker"),
        );
    }
    let stop = Arc::new(AtomicBool::new(false));
    std::thread::scope(|scope| {
        let _guard = Stop(Arc::clone(&stop));
        for (worker, listener) in workers.iter().zip(&listeners) {
            let stop = &stop;
            scope.spawn(move || {
                worker
                    .serve(listener, None, &|| stop.load(Ordering::Relaxed))
                    .expect("serve")
            });
        }
        let coordinator = Coordinator::new(
            Options::new(addresses, key.0.clone()),
            b"cancel model",
            &store,
        )
        .expect("coordinator");
        let out = crate::explore_distributed(
            &plan,
            &store,
            crate::Config {
                opts: Opts::default(),
                max_states: None,
                max_depth: None,
                workers: 1,
                warn: true,
                sample: None,
            },
            &|| coordinator.usage().membership_queries > 0,
            None,
            &crate::storage::Storage::default(),
            &coordinator,
        );
        assert!(
            matches!(out.verdict, crate::Verdict::Budget),
            "{:?}",
            out.verdict
        );
        assert!(out.trace.is_none());
        let facts = coordinator.usage();
        assert!(facts.membership_queries > 0);
        assert!(facts.completed_units < facts.assigned_units);
    });
}
