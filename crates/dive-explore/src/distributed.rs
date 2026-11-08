//! Authenticated, bounded native pure-safety work over trusted TCP endpoints.
//!
//! HMAC authenticates identity and integrity; it does not encrypt traffic.
//! Operators supply a private network or tunnel when confidentiality is needed.
//! Received units contain values for an already compiled plan, never programs,
//! executable paths, host grants or model-selection instructions.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use dive_core::owned_exchange::{Exchange, Startup};
use dive_core::{Error, Handle, Heap, Machine, Opts, Plan, Store};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod wire;

/// Private protocol frame admission, independent of ordinary local checking.
pub const MAX_FRAME_BYTES: usize = 64 << 20;
/// Private owned-value DAG admission per work or result.
pub const MAX_DAG_BYTES: usize = 32 << 20;
/// Actual source-emission admission; crossing it cannot certify completion.
pub const MAX_SUCCESSORS: u64 = 16_384;
const KEY_BYTES: usize = 1 << 20;
const MAGIC: &[u8; 8] = b"DIVEWN01";
const DOMAIN: &[u8] = b"dive worker HMAC-SHA256 v1\0";
/// Shortest and longest wait between accept polls on an idle worker listener.
const ACCEPT_POLL_MIN: Duration = Duration::from_micros(50);
const ACCEPT_POLL_MAX: Duration = Duration::from_millis(10);
/// Wait between accept polls: starts at [`ACCEPT_POLL_MIN`] after a session
/// and doubles on each empty poll up to [`ACCEPT_POLL_MAX`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AcceptBackoff(Duration);

impl Default for AcceptBackoff {
    fn default() -> Self {
        Self(ACCEPT_POLL_MIN)
    }
}

impl AcceptBackoff {
    /// Returns the wait for this empty poll and doubles the next one, capped.
    fn next_wait(&mut self) -> Duration {
        let wait = self.0;
        self.0 = (wait * 2).min(ACCEPT_POLL_MAX);
        wait
    }

    /// Restarts from the shortest wait after a session arrives.
    fn reset(&mut self) {
        self.0 = ACCEPT_POLL_MIN;
    }
}

const PING: u8 = 1;
const PONG: u8 = 2;
const WORK: u8 = 3;
const QUERY: u8 = 4;
const ANSWER: u8 = 5;
const RESULT: u8 = 6;
const FAILED: u8 = 7;

fn failure(message: impl std::fmt::Display) -> Error {
    Error::internal(format!("DIVE-S0500: distributed safety: {message}"))
}
fn io(error: impl std::fmt::Display) -> Error {
    Error::internal(format!("DIVE-S0502: distributed I/O: {error}"))
}
fn random() -> Result<[u8; 32], Error> {
    let mut value = [0; 32];
    getrandom::fill(&mut value).map_err(failure)?;
    Ok(value)
}

fn remaining(deadline: Instant) -> Result<Duration, Error> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|n| !n.is_zero())
        .ok_or_else(|| {
            io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "authenticated unit deadline",
            ))
        })
}
// A socket timeout surfaces as WouldBlock on Unix; name the deadline instead.
fn timed_out(error: &std::io::Error) -> String {
    if matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ) {
        "no reply within the --worker-timeout-ms deadline".into()
    } else {
        error.to_string()
    }
}
// Refresh the remaining absolute deadline for every syscall. A peer trickling
// bytes cannot extend a unit by repeatedly resetting a relative socket timeout.
fn read_until(
    stream: &mut TcpStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), Error> {
    while !bytes.is_empty() {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(io)?;
        match stream.read(bytes) {
            Ok(0) => {
                return Err(io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "incomplete authenticated frame",
                )));
            }
            Ok(n) => bytes = &mut bytes[n..],
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io(timed_out(&e))),
        }
    }
    Ok(())
}
fn write_until(stream: &mut TcpStream, mut bytes: &[u8], deadline: Instant) -> Result<(), Error> {
    while !bytes.is_empty() {
        stream
            .set_write_timeout(Some(remaining(deadline)?))
            .map_err(io)?;
        match stream.write(bytes) {
            Ok(0) => {
                return Err(io(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "incomplete authenticated frame",
                )));
            }
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io(timed_out(&e))),
        }
    }
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(&mut text, "{byte:02x}");
            text
        })
}

struct Secret([u8; 32]);
impl Secret {
    fn read(path: &Path) -> Result<Self, Error> {
        let mut bytes = Vec::new();
        let named = |e: std::io::Error| io(format!("worker key file {}: {e}", path.display()));
        std::fs::File::open(path)
            .map_err(named)?
            .take(33)
            .read_to_end(&mut bytes)
            .map_err(named)?;
        let key = bytes.try_into().map_err(|_| {
            failure(format!(
                "worker key file {} must contain exactly 32 bytes",
                path.display()
            ))
        })?;
        Ok(Self(key))
    }
    fn mac(&self, parts: &[&[u8]]) -> Result<Hmac<Sha256>, Error> {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.0).map_err(failure)?;
        mac.update(DOMAIN);
        for part in parts {
            mac.update(part);
        }
        Ok(mac)
    }
    fn tag(&self, parts: &[&[u8]]) -> Result<Vec<u8>, Error> {
        Ok(self.mac(parts)?.finalize().into_bytes().to_vec())
    }
    fn verify(&self, parts: &[&[u8]], tag: &[u8]) -> Result<(), Error> {
        self.mac(parts)?
            .verify_slice(tag)
            .map_err(|_| failure("authentication failed"))
    }
}

/// Explicit coordinator endpoints and resource policy. Key bytes are read from
/// the named local file and never included in reports, arguments or Debug values.
#[derive(Clone, Debug)]
pub struct Options {
    /// Explicit trusted worker listeners; at least two distinct endpoints.
    pub workers: Vec<SocketAddr>,
    /// Local path to the shared 32-byte authentication secret.
    pub key_file: PathBuf,
    /// Per-network-operation and per-worker-unit bound.
    pub timeout: Duration,
    /// Retries of an uncommitted pure source after connection loss, at most eight.
    pub retries: u8,
}
impl Options {
    /// Two-or-more-worker policy with five-second units and two retries.
    #[must_use]
    pub fn new(workers: Vec<SocketAddr>, key_file: PathBuf) -> Self {
        Self {
            workers,
            key_file,
            timeout: Duration::from_secs(5),
            retries: 2,
        }
    }
    fn validate(&self) -> Result<(), Error> {
        let distinct: HashSet<_> = self.workers.iter().collect();
        if distinct.len() != self.workers.len()
            || !(2..=64).contains(&distinct.len())
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(60)
            || self.retries > 8
        {
            return Err(failure(
                "require 2..64 distinct workers, nonzero <=60s unit timeout and <=8 retries",
            ));
        }
        Ok(())
    }
}

/// Authenticated worker process facts, not remote attestation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerInfo {
    /// Fresh independently initialized worker identity.
    pub id: String,
    /// Worker-reported operating-system process id.
    pub process_id: u32,
    /// Complete WORK frames sent to this authenticated instance, including retries.
    pub sent_units: u64,
    /// Complete authenticated RESULT frames received, before structural admission.
    pub result_units: u64,
}
/// Actual work/retry facts; logical model counters include only ordered commit.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Requested trusted endpoints.
    pub configured_workers: usize,
    /// Independently authenticated process identities actually contacted.
    pub workers: Vec<WorkerInfo>,
    /// Source units assigned (excluding retries and startup pings).
    pub assigned_units: u64,
    /// Complete source results imported/admitted at ordered consumption.
    pub completed_units: u64,
    /// Actual connection-loss retries of uncommitted work.
    pub retries: u64,
    /// Largest completed source level/epoch.
    pub epoch: u64,
    /// Membership queries against the batch's real coordinator seen set.
    pub membership_queries: u64,
    /// Authenticated duplicate frames ignored without committing work twice.
    pub duplicate_frames: u64,
    /// Units left incomplete after retry policy, including unused later work.
    pub failed_units: u64,
    /// Required transport/admission failures reached in source order.
    pub failures: Vec<String>,
}

#[derive(Clone, Copy)]
struct Ticket {
    epoch: u64,
    work: u64,
    attempt: u64,
}
#[derive(Clone, Copy)]
struct Binding {
    identity: [u8; 32],
    run: [u8; 32],
    client: [u8; 32],
    server: [u8; 32],
    ticket: Ticket,
}
impl Binding {
    fn hello(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&self.identity);
        bytes.extend_from_slice(&self.run);
        bytes.extend_from_slice(&self.client);
        for n in [self.ticket.epoch, self.ticket.work, self.ticket.attempt] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        bytes
    }
    fn context(&self) -> Vec<u8> {
        let mut bytes = self.hello();
        bytes.extend_from_slice(&self.server);
        bytes
    }
}

struct Channel {
    stream: TcpStream,
    binding: Binding,
    send: u64,
    receive: u64,
    coordinator: bool,
    deadline: Instant,
}
impl Channel {
    fn direction(coordinator: bool) -> &'static [u8] {
        if coordinator {
            b"coordinator frame"
        } else {
            b"worker frame"
        }
    }
    fn write(&mut self, key: &Secret, kind: u8, payload: &[u8]) -> Result<(), Error> {
        if payload.len() > MAX_FRAME_BYTES {
            return Err(failure("frame byte admission"));
        }
        let mut header = [0; 17];
        header[0] = kind;
        header[1..9].copy_from_slice(&self.send.to_le_bytes());
        header[9..].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        let tag = key.tag(&[
            Self::direction(self.coordinator),
            &self.binding.context(),
            &header,
            payload,
        ])?;
        write_until(&mut self.stream, &header, self.deadline)?;
        write_until(&mut self.stream, payload, self.deadline)?;
        write_until(&mut self.stream, &tag, self.deadline)?;
        self.send = self
            .send
            .checked_add(1)
            .ok_or_else(|| failure("frame sequence exhausted"))?;
        Ok(())
    }
    fn read(&mut self, key: &Secret, usage: &Mutex<Usage>) -> Result<(u8, Vec<u8>), Error> {
        for _ in 0..64 {
            let mut header = [0; 17];
            read_until(&mut self.stream, &mut header, self.deadline)?;
            let seq = u64::from_le_bytes(header[1..9].try_into().map_err(failure)?);
            let len = usize::try_from(u64::from_le_bytes(header[9..].try_into().map_err(failure)?))
                .map_err(failure)?;
            if len > MAX_FRAME_BYTES {
                return Err(failure("frame byte admission"));
            }
            let mut payload = Vec::new();
            payload.try_reserve_exact(len).map_err(failure)?;
            payload.resize(len, 0);
            read_until(&mut self.stream, &mut payload, self.deadline)?;
            let mut tag = [0; 32];
            read_until(&mut self.stream, &mut tag, self.deadline)?;
            key.verify(
                &[
                    Self::direction(!self.coordinator),
                    &self.binding.context(),
                    &header,
                    &payload,
                ],
                &tag,
            )?;
            if seq < self.receive {
                usage
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .duplicate_frames += 1;
                continue;
            }
            if seq != self.receive {
                return Err(failure("future or stale session sequence"));
            }
            self.receive = self
                .receive
                .checked_add(1)
                .ok_or_else(|| failure("frame sequence exhausted"))?;
            return Ok((header[0], payload));
        }
        Err(failure("duplicate frame admission"))
    }
}

fn timeouts(stream: &TcpStream, timeout: Duration) -> Result<(), Error> {
    stream
        .set_nodelay(true)
        .and_then(|()| stream.set_read_timeout(Some(timeout)))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(io)
}
fn connect(
    address: SocketAddr,
    key: &Secret,
    identity: [u8; 32],
    run: [u8; 32],
    ticket: Ticket,
    timeout: Duration,
) -> Result<(Channel, WorkerInfo), Error> {
    let deadline = Instant::now() + timeout;
    let mut stream = TcpStream::connect_timeout(&address, timeout).map_err(io)?;
    timeouts(&stream, timeout)?;
    let mut binding = Binding {
        identity,
        run,
        client: random()?,
        server: [0; 32],
        ticket,
    };
    let hello = binding.hello();
    let tag = key.tag(&[b"hello", &hello])?;
    write_until(&mut stream, &hello, deadline)?;
    write_until(&mut stream, &tag, deadline)?;
    let mut ack = [0; 69];
    let mut mac = [0; 32];
    // A worker drops a session whose HELLO fails authentication without an
    // authenticated reply, so name the likely causes of that silence.
    read_until(&mut stream, &mut ack, deadline)
        .and_then(|()| read_until(&mut stream, &mut mac, deadline))
        .map_err(|error| {
            io(format!(
                "worker {address} closed or did not answer the authenticated handshake \
                 (check that every role reads the same --worker-key file and runs the \
                 same dive build, and that the worker is running): {}",
                error
                    .message()
                    .trim_start_matches("DIVE-S0502: distributed I/O: ")
            ))
        })?;
    key.verify(&[b"ack", &hello, &ack], &mac)?;
    if ack[0] != 0 {
        return Err(failure(
            "worker source/config/executable/policy/startup identity mismatch",
        ));
    }
    binding.server.copy_from_slice(&ack[1..33]);
    let info = WorkerInfo {
        id: hex(&ack[33..65]),
        process_id: u32::from_le_bytes(ack[65..].try_into().map_err(failure)?),
        sent_units: 0,
        result_units: 0,
    };
    Ok((
        Channel {
            stream,
            binding,
            send: 0,
            receive: 0,
            coordinator: true,
            deadline,
        },
        info,
    ))
}
fn accept(
    mut stream: TcpStream,
    key: &Secret,
    identity: [u8; 32],
    worker: [u8; 32],
    timeout: Duration,
) -> Result<Channel, Error> {
    let deadline = Instant::now() + timeout;
    timeouts(&stream, timeout)?;
    let mut hello = [0; 128];
    let mut tag = [0; 32];
    read_until(&mut stream, &mut hello, deadline)?;
    read_until(&mut stream, &mut tag, deadline)?;
    key.verify(&[b"hello", &hello], &tag)?;
    if &hello[..8] != MAGIC {
        return Err(failure("worker protocol version"));
    }
    if hello[8..40] != identity {
        let mut rejection = [0; 69];
        rejection[0] = 1;
        let tag = key.tag(&[b"ack", &hello, &rejection])?;
        write_until(&mut stream, &rejection, deadline)?;
        write_until(&mut stream, &tag, deadline)?;
        return Err(failure(
            "source/config/executable/policy/startup identity mismatch",
        ));
    }
    let word = |at| -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            hello[at..at + 8].try_into().map_err(failure)?,
        ))
    };
    let mut binding = Binding {
        identity,
        run: hello[40..72].try_into().map_err(failure)?,
        client: hello[72..104].try_into().map_err(failure)?,
        server: random()?,
        ticket: Ticket {
            epoch: word(104)?,
            work: word(112)?,
            attempt: word(120)?,
        },
    };
    let mut ack = [0; 69];
    ack[1..33].copy_from_slice(&binding.server);
    ack[33..65].copy_from_slice(&worker);
    ack[65..].copy_from_slice(&std::process::id().to_le_bytes());
    let tag = key.tag(&[b"ack", &hello, &ack])?;
    write_until(&mut stream, &ack, deadline)?;
    write_until(&mut stream, &tag, deadline)?;
    // Keep only independently checked fixed-size identity/session fields.
    binding.identity = identity;
    Ok(Channel {
        stream,
        binding,
        send: 0,
        receive: 0,
        coordinator: false,
        deadline,
    })
}

fn identity(base: &[u8], store: &Store) -> Result<([u8; 32], Vec<u8>), Error> {
    store.strings().seal();
    let image = dive_core::store::snapshot::capture(store, MAX_DAG_BYTES, &|| true)?;
    let mut hash = Sha256::new();
    hash.update(b"dive distributed pure core 0151 v1\0");
    hash.update((base.len() as u64).to_le_bytes());
    hash.update(base);
    hash.update(&image);
    Ok((hash.finalize().into(), image))
}

/// One coordinator's authenticated session and immutable endpoint policy.
pub struct Coordinator {
    options: Options,
    key: Secret,
    identity: [u8; 32],
    run: [u8; 32],
    startup: Startup,
    usage: Mutex<Usage>,
}

// Bounded reply bytes plus the matching issuing authority. Import only when
// this source reaches ordered consumption, preserving Store/history authority.
pub(crate) struct Received {
    exchange: Exchange,
    payload: Result<Vec<u8>, Error>,
    epoch: u64,
}
impl Coordinator {
    /// Authenticates all endpoints against this compiled model before search.
    ///
    /// # Errors
    /// Policy, identity, authentication, framing, startup admission or I/O failure.
    pub fn new(options: Options, base_identity: &[u8], store: &Arc<Store>) -> Result<Self, Error> {
        options.validate()?;
        let key = Secret::read(&options.key_file)?;
        let (identity, _) = identity(base_identity, store)?;
        let run = random()?;
        let startup = Startup::new(
            &Heap::new(Arc::clone(store), true, false),
            MAX_DAG_BYTES,
            &|| true,
        )?;
        let owner = Self {
            usage: Mutex::new(Usage {
                configured_workers: options.workers.len(),
                ..Usage::default()
            }),
            options,
            key,
            identity,
            run,
            startup,
        };
        for &address in &owner.options.workers {
            let (mut channel, info) = connect(
                address,
                &owner.key,
                identity,
                run,
                Ticket {
                    epoch: 0,
                    work: u64::MAX,
                    attempt: 0,
                },
                owner.options.timeout,
            )?;
            channel.write(&owner.key, PING, &[])?;
            if channel.read(&owner.key, &owner.usage)?.0 != PONG {
                return Err(failure("startup acknowledgement"));
            }
            owner.worker(info);
        }
        if owner.usage().workers.len() < 2 {
            return Err(failure("two independently initialized workers required"));
        }
        Ok(owner)
    }
    fn worker(&self, info: WorkerInfo) {
        let mut usage = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
        if !usage.workers.iter().any(|w| w.id == info.id) {
            usage.workers.push(info);
        }
    }
    fn unit_fact(&self, id: &str, result: bool) -> Result<(), Error> {
        let mut usage = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
        let info = usage
            .workers
            .iter_mut()
            .find(|worker| worker.id == id)
            .ok_or_else(|| failure("unregistered authenticated worker"))?;
        let count = if result {
            &mut info.result_units
        } else {
            &mut info.sent_units
        };
        *count = count
            .checked_add(1)
            .ok_or_else(|| failure("worker unit counter overflow"))?;
        Ok(())
    }
    /// Actual contacted processes, work and retry facts.
    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    pub(crate) fn record_failure(&self, error: &Error) {
        if error.message().starts_with("DIVE-S05") {
            let mut facts = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
            if !facts
                .failures
                .iter()
                .any(|failure| failure == error.message())
            {
                facts.failures.push(error.message().into());
            }
        }
    }
    pub(crate) fn width(&self) -> usize {
        self.options.workers.len()
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one bounded batch borrows the actual committed seen authority"
    )]
    pub(crate) fn expand(
        &self,
        plan: &Plan,
        store: &Arc<Store>,
        rows: &[Handle],
        seen: &crate::Seen,
        epoch: u64,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<Option<Received>>, Error> {
        let width = plan.vars.len();
        if width == 0 || !rows.len().is_multiple_of(width) || rows.len() / width > self.width() {
            return Err(failure("remote batch dimensions"));
        }
        let heap = Heap::new(Arc::clone(store), true, false);
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(rows.len() / width)
            .map_err(failure)?;
        for row in rows.chunks_exact(width) {
            jobs.push(Exchange::request_with_startup(
                &heap,
                row,
                MAX_DAG_BYTES,
                &|| !poll(),
                &self.startup,
            )?);
        }
        let first = {
            let mut facts = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
            let first = facts.assigned_units;
            facts.assigned_units = facts
                .assigned_units
                .checked_add(jobs.len() as u64)
                .ok_or_else(|| failure("work ids exhausted"))?;
            first
        };
        let received = std::thread::scope(|scope| {
            let handles: Vec<_> = jobs
                .into_iter()
                .enumerate()
                .map(|(i, (bytes, exchange))| {
                    std::thread::Builder::new()
                        .name("dive-network".into())
                        .spawn_scoped(scope, move || {
                            (
                                exchange,
                                self.job(first + i as u64, epoch, &bytes, seen, store, poll),
                            )
                        })
                        .map_err(|error| failure(format!("network thread start: {error}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .map_err(|_| failure("network worker thread failed"))
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        let mut result = Vec::new();
        result.try_reserve_exact(received.len()).map_err(failure)?;
        for (exchange, payload) in received {
            result.push(Some(Received {
                exchange,
                payload,
                epoch,
            }));
        }
        Ok(result)
    }

    pub(crate) fn import(
        &self,
        received: Received,
        plan: &Plan,
        heap: &mut Heap,
        poll: &(dyn Fn() -> bool + Sync),
        metrics: &mut crate::Metrics,
    ) -> Result<crate::parallel::Expanded, Error> {
        let payload = received.payload?;
        let (meta, dag) = wire::split(&payload)?;
        let roots = received
            .exchange
            .receive_reply(heap, dag, MAX_DAG_BYTES, &|| !poll())?;
        let (expanded, counts) = wire::decode(
            meta,
            &roots,
            plan,
            usize::try_from(MAX_SUCCESSORS).map_err(failure)?,
        )?;
        counts.apply(metrics)?;
        let mut facts = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
        facts.completed_units += 1;
        facts.epoch = facts.epoch.max(received.epoch);
        Ok(expanded)
    }

    fn job(
        &self,
        work: u64,
        epoch: u64,
        bytes: &[u8],
        seen: &crate::Seen,
        store: &Arc<Store>,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u8>, Error> {
        let mut final_error = failure("work was not attempted");
        for attempt in 0..=self.options.retries {
            if poll() {
                return Err(Error::stop());
            }
            if attempt > 0 {
                self.usage
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .retries += 1;
            }
            let index = usize::try_from(work % (self.width() as u64)).map_err(failure)?;
            let address = self.options.workers[(index + usize::from(attempt)) % self.width()];
            let result = (|| {
                let (mut channel, info) = connect(
                    address,
                    &self.key,
                    self.identity,
                    self.run,
                    Ticket {
                        epoch,
                        work,
                        attempt: u64::from(attempt),
                    },
                    self.options.timeout,
                )?;
                let worker_id = info.id.clone();
                self.worker(info);
                channel.write(&self.key, WORK, bytes)?;
                self.unit_fact(&worker_id, false)?;
                let heap = Heap::new(Arc::clone(store), true, false);
                loop {
                    if poll() {
                        return Err(Error::stop());
                    }
                    let (kind, payload) = channel.read(&self.key, &self.usage)?;
                    match kind {
                        QUERY => {
                            if payload.len() < 8 || payload.len() > 8 + KEY_BYTES {
                                return Err(failure("membership query admission"));
                            }
                            let key = u64::from_le_bytes(payload[..8].try_into().map_err(failure)?);
                            let present = if let Some(exact) = &seen.exact {
                                exact.contains_encoded(
                                    key,
                                    &payload[8..],
                                    &heap,
                                    KEY_BYTES,
                                    poll,
                                )?
                            } else {
                                seen.contains(key)
                            };
                            self.usage
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .membership_queries += 1;
                            channel.write(&self.key, ANSWER, &[u8::from(present)])?;
                        }
                        RESULT => {
                            self.unit_fact(&worker_id, true)?;
                            return Ok(payload);
                        }
                        FAILED => {
                            return Err(failure(format!(
                                "worker refused incomplete unit: {}",
                                String::from_utf8_lossy(&payload)
                            )));
                        }
                        _ => return Err(failure("unexpected worker message")),
                    }
                }
            })();
            match result {
                Ok(bytes) => return Ok(bytes),
                Err(error) if error.kind() == dive_core::ErrorKind::Stop => return Err(error),
                Err(error) => {
                    let retry = error.message().starts_with("DIVE-S0502:");
                    final_error = error;
                    if !retry {
                        break;
                    }
                }
            }
        }
        self.usage
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failed_units += 1;
        Err(final_error)
    }
}

/// Independently prepared native worker. Its plan is fixed before any session.
pub struct Worker {
    plan: Plan,
    store: Arc<Store>,
    startup: Vec<u8>,
    opts: Opts,
    key: Secret,
    identity: [u8; 32],
    id: [u8; 32],
    timeout: Duration,
}
impl Worker {
    /// Binds the compiled startup and authentication secret for pure work.
    ///
    /// # Errors
    /// Effects, quotient modes, private startup admission, keys or identity hashing.
    pub fn new(
        plan: Plan,
        store: Arc<Store>,
        opts: Opts,
        base_identity: &[u8],
        key_file: &Path,
        timeout: Duration,
    ) -> Result<Self, Error> {
        if plan.effects.any || plan.view.is_some() || plan.symmetry.is_some() {
            return Err(Error::unsupported("distributed effects, VIEW or symmetry"));
        }
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(failure("worker timeout must be nonzero and <=60s"));
        }
        let (identity, startup) = identity(base_identity, &store)?;
        Ok(Self {
            plan,
            store,
            startup,
            opts,
            key: Secret::read(key_file)?,
            identity,
            id: random()?,
            timeout,
        })
    }
    /// Serves bounded authenticated units from a supplied listener until stopped.
    /// `max_units` permits finite process lifetimes without a remote shutdown grant.
    ///
    /// # Errors
    /// Listener ownership or setup failure. Bad sessions are rejected and counted.
    pub fn serve(
        &self,
        listener: &TcpListener,
        max_units: Option<u64>,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Usage, Error> {
        listener.set_nonblocking(true).map_err(io)?;
        let usage = Mutex::new(Usage::default());
        // The coordinator opens one session per source unit, so the next one
        // usually arrives within a millisecond of the last. Poll quickly after
        // a session and back off to the idle interval while nothing arrives.
        let mut idle = AcceptBackoff::default();
        while !stop()
            && max_units.is_none_or(|n| {
                usage
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .completed_units
                    < n
            })
        {
            let stream = match listener.accept() {
                Ok((s, _)) => {
                    idle.reset();
                    s
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(idle.next_wait());
                    continue;
                }
                Err(e) => return Err(io(e)),
            };
            let result = accept(stream, &self.key, self.identity, self.id, self.timeout).and_then(
                |mut channel| {
                    let result = self.unit(&mut channel, &usage);
                    if let Err(error) = &result {
                        let _ = channel.write(&self.key, FAILED, error.message().as_bytes());
                    }
                    result
                },
            );
            if let Err(error) = result {
                let mut facts = usage.lock().unwrap_or_else(PoisonError::into_inner);
                if facts.failures.len() < 64 {
                    facts.failures.push(error.message().into());
                }
            }
        }
        Ok(usage.into_inner().unwrap_or_else(PoisonError::into_inner))
    }
    fn unit(&self, channel: &mut Channel, usage: &Mutex<Usage>) -> Result<(), Error> {
        let (kind, payload) = channel.read(&self.key, usage)?;
        if kind == PING && payload.is_empty() {
            return channel.write(&self.key, PONG, &[]);
        }
        if kind != WORK {
            return Err(failure("work kind"));
        }
        let deadline = channel.deadline;
        let poll = || Instant::now() >= deadline;
        let work = (|| {
            let store = dive_core::store::snapshot::restore(
                &self.store,
                &self.startup,
                &self.startup,
                MAX_DAG_BYTES,
                &|| !poll(),
            )?;
            let mut machine = Machine::new(
                store,
                self.plan.vars.len(),
                self.opts,
                dive_core::memo::Memo::new(
                    &self
                        .plan
                        .code
                        .sites
                        .iter()
                        .map(dive_core::plan::MemoSite::key_len)
                        .collect::<Vec<_>>(),
                ),
            );
            machine.set_work_control(Some(Arc::new(Deadline(deadline))));
            let startup = Startup::new(&machine.heap, MAX_DAG_BYTES, &|| !poll())?;
            let (source, exchange) = Exchange::receive_request_with_startup(
                &mut machine.heap,
                &payload,
                MAX_DAG_BYTES,
                &|| !poll(),
                &startup,
            )?;
            if source.len() != self.plan.vars.len() || source.iter().any(|h| h.is_empty()) {
                return Err(failure("source row width/completeness"));
            }
            let mut member = RemoteMember {
                channel,
                key: &self.key,
                usage,
            };
            let mut metrics = crate::Metrics::default();
            let expanded = crate::parallel::expand_remote(
                &mut machine,
                &self.plan,
                &source,
                MAX_SUCCESSORS,
                &poll,
                &mut metrics,
                &mut member,
            );
            let (meta, roots) = wire::encode(&expanded, &metrics, MAX_FRAME_BYTES)?;
            let dag = exchange.reply(&machine.heap, &roots, MAX_DAG_BYTES, &|| !poll())?;
            wire::join(&meta, &dag, MAX_FRAME_BYTES)
        })();
        match work {
            Ok(bytes) => {
                channel.write(&self.key, RESULT, &bytes)?;
                usage
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .completed_units += 1;
                Ok(())
            }
            Err(error) => {
                let mut facts = usage.lock().unwrap_or_else(PoisonError::into_inner);
                if facts.failures.len() < 64 {
                    facts.failures.push(error.message().into());
                }
                drop(facts);
                // The unit deadline may already have passed; give the bounded
                // FAILED report its own deadline so the coordinator learns why.
                channel.deadline = Instant::now() + self.timeout;
                let message = if Instant::now() >= deadline {
                    format!(
                        "the unit did not finish within the worker's --worker-timeout-ms of {} ms",
                        self.timeout.as_millis()
                    )
                } else {
                    error.message().to_owned()
                };
                channel.write(&self.key, FAILED, message.as_bytes())
            }
        }
    }
}

#[derive(Debug)]
struct Deadline(Instant);
impl dive_core::eval::WorkControl for Deadline {
    fn proceed(&self) -> bool {
        Instant::now() < self.0
    }
}

struct RemoteMember<'a> {
    channel: &'a mut Channel,
    key: &'a Secret,
    usage: &'a Mutex<Usage>,
}
impl crate::parallel::Membership for RemoteMember<'_> {
    fn contains(&mut self, key: u64, identity: &[Handle], heap: &Heap) -> Result<bool, Error> {
        let bytes = dive_core::key::canonical(heap, identity, KEY_BYTES, &|| {
            Instant::now() < self.channel.deadline
        })
        .map_err(|e| failure(format!("canonical worker key: {e:?}")))?;
        let mut query = Vec::new();
        query.extend_from_slice(&key.to_le_bytes());
        query.extend_from_slice(&bytes);
        self.channel.write(self.key, QUERY, &query)?;
        let (kind, answer) = self.channel.read(self.key, self.usage)?;
        if kind != ANSWER {
            return Err(failure("membership answer kind"));
        }
        match answer.as_slice() {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(failure("membership answer")),
        }
    }
}

#[cfg(test)]
mod fault_tests;

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "local authenticated worker backend controls"
    )]
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    struct Stop(Arc<AtomicBool>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    #[test]
    fn accept_backoff_doubles_to_the_cap_and_resets_after_a_session() {
        let mut backoff = AcceptBackoff::default();
        let waits: Vec<Duration> = (0..12).map(|_| backoff.next_wait()).collect();
        assert_eq!(waits[0], ACCEPT_POLL_MIN);
        assert_eq!(waits[1], ACCEPT_POLL_MIN * 2);
        assert!(waits.windows(2).all(|w| w[1] >= w[0]));
        assert!(waits.iter().all(|w| *w <= ACCEPT_POLL_MAX));
        // 50 us doubled eight times exceeds 10 ms, so the cap holds from there on.
        assert_eq!(waits[8], ACCEPT_POLL_MAX);
        assert_eq!(waits[11], ACCEPT_POLL_MAX);
        backoff.reset();
        assert_eq!(backoff.next_wait(), ACCEPT_POLL_MIN);
    }

    #[test]
    fn two_independent_native_workers_complete_actual_ordered_search() {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        for exact in [false, true] {
            let path = std::env::temp_dir().join(format!(
                "dive-worker-key-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(&path, [37; 32]).expect("test key");
            let store = Arc::new(Store::new(true));
            let plan = crate::tests::frontier_plan(&store, false).expect("coordinator plan");
            let local = crate::explore_with_storage(
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
                &|| false,
                None,
                None,
                &crate::storage::Storage {
                    exact,
                    ..crate::storage::Storage::default()
                },
            );
            let mut workers = Vec::new();
            let mut listeners = Vec::new();
            let mut endpoints = Vec::new();
            for _ in 0..2 {
                let listener = TcpListener::bind("127.0.0.1:0").expect("local listener");
                endpoints.push(listener.local_addr().expect("address"));
                let owner = Arc::new(Store::new(true));
                let plan = crate::tests::frontier_plan(&owner, false).expect("independent plan");
                workers.push(
                    Worker::new(
                        plan,
                        owner,
                        Opts::default(),
                        b"same independently compiled test",
                        &path,
                        Duration::from_secs(5),
                    )
                    .expect("worker"),
                );
                listeners.push(listener);
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
                    Options::new(endpoints, path.clone()),
                    b"same independently compiled test",
                    &store,
                )
                .expect("authenticated coordinator");
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
                    &|| false,
                    None,
                    &crate::storage::Storage {
                        exact,
                        ..crate::storage::Storage::default()
                    },
                    &coordinator,
                );
                assert_eq!(out.verdict, local.verdict, "{:?}", out.verdict);
                assert_eq!(out.stats, local.stats);
                let facts = coordinator.usage();
                assert_eq!(facts.workers.len(), 2);
                assert_eq!(facts.completed_units, 128);
                assert_eq!(facts.assigned_units, 128);
                assert_eq!(facts.membership_queries, 128);
                assert_eq!(facts.failures, [] as [String; 0]);
            });
            std::fs::remove_file(path).expect("remove key");
        }
    }

    #[test]
    fn authentication_binds_identity_run_epoch_work_attempt_and_payload() {
        let key = Secret([17; 32]);
        let binding = Binding {
            identity: [1; 32],
            run: [2; 32],
            client: [3; 32],
            server: [4; 32],
            ticket: Ticket {
                epoch: 5,
                work: 6,
                attempt: 7,
            },
        };
        let context = binding.context();
        let tag = key
            .tag(&[&context, b"work", b"actual payload"])
            .expect("tag");
        assert!(
            key.verify(&[&context, b"work", b"actual payload"], &tag)
                .is_ok()
        );
        for at in [8, 40, 72, 104, 112, 120, 128] {
            let mut altered = context.clone();
            altered[at] ^= 1;
            assert!(
                key.verify(&[&altered, b"work", b"actual payload"], &tag)
                    .is_err()
            );
        }
        assert!(
            key.verify(&[&context, b"result", b"actual payload"], &tag)
                .is_err()
        );
        assert!(
            key.verify(&[&context, b"work", b"altered payload"], &tag)
                .is_err()
        );
    }
}
