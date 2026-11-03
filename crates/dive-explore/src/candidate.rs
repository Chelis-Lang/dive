//! Ordered candidate occurrence and owned-value payload queues.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use dive_core::candidate_codec::CandidateCodec;
use dive_core::{Error, ErrorKind, Handle, Machine, TlcClass};
use dive_store::spill::{MAX_RECORD_BYTES, OrderedFrontier};

use crate::ending::Site;
use crate::parallel::{Inv, Occ};
use crate::storage::CandidateUsage;

pub(crate) type Account = Arc<Mutex<CandidateUsage>>;

#[derive(Debug)]
pub(crate) struct Spool {
    codec: CandidateCodec,
    queue: OrderedFrontier,
    pending: Vec<u8>,
    account: Account,
    retained: u64,
    written: u64,
    identity_width: usize,
}

impl Spool {
    pub(crate) fn new(
        m: &Machine,
        bytes: usize,
        directory: &Path,
        account: Account,
        identity_width: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            codec: CandidateCodec::new(&m.heap)?,
            queue: OrderedFrontier::new(bytes, directory),
            pending: Vec::new(),
            account,
            retained: 0,
            written: 0,
            identity_width,
        })
    }
    pub(crate) fn keep(&mut self, m: &mut Machine, roots: &[Handle]) -> Result<usize, Error> {
        if !self.pending.is_empty() {
            return Err(Error::internal("uncommitted candidate payload"));
        }
        self.pending = self.codec.encode(&mut m.heap, roots, MAX_RECORD_BYTES)?;
        Ok(0)
    }
    pub(crate) fn len(&self) -> usize {
        self.queue.len()
    }
    pub(crate) fn push(&mut self, action: usize, occ: &Occ) -> Result<(), Error> {
        let mut out = Write { bytes: Vec::new() };
        out.word(action as u64)?;
        match occ {
            Occ::Old { key, inside, at } => {
                out.byte(0)?;
                out.word(*key)?;
                out.byte(u8::from(*inside))?;
                out.byte(u8::from(at.is_some()))?;
            }
            Occ::CaptureLimit => out.byte(1)?,
            Occ::CaptureBudget => out.byte(2)?,
            Occ::Cand {
                key,
                in_model,
                inv,
                imp,
                ..
            } => {
                out.byte(3)?;
                out.word(*key)?;
                out.byte(u8::from(*in_model))?;
                out.inv(inv)?;
                out.inv(imp)?;
            }
            Occ::Incomplete { .. } => out.byte(4)?,
            Occ::CheckErr { site, error, .. } => {
                out.byte(5)?;
                out.site(*site)?;
                out.error(error)?;
            }
        }
        out.put(&self.pending)?;
        self.pending.clear();
        let result = self.queue.push(self.queue.len() as u64, &out.bytes);
        self.refresh();
        result.map_err(|error| self.failure(error.to_string()))
    }
    pub(crate) fn read(&mut self, index: usize, m: &mut Machine) -> Result<(usize, Occ), Error> {
        let (id, bytes) = self
            .queue
            .read(index)
            .map_err(|error| self.failure(error.to_string()))?;
        if id != index as u64 {
            return Err(self.failure("candidate ordinal differs from position".into()));
        }
        let mut input = Read {
            bytes: &bytes,
            at: 0,
        };
        let action = input.index()?;
        let mut occ = match input.byte()? {
            0 => Occ::Old {
                key: input.word()?,
                inside: input.boolean()?,
                at: input.boolean()?.then_some(0),
            },
            1 => Occ::CaptureLimit,
            2 => Occ::CaptureBudget,
            3 => Occ::Cand {
                key: input.word()?,
                identity: Vec::new(),
                in_model: input.boolean()?,
                inv: input.inv()?,
                imp: input.inv()?,
                at: 0,
            },
            4 => Occ::Incomplete { at: 0 },
            5 => Occ::CheckErr {
                site: input.site()?,
                error: input.error()?,
                at: 0,
            },
            _ => return Err(self.failure("unknown occurrence tag".into())),
        };
        let needs_values = match &occ {
            Occ::Old { at, .. } => at.is_some(),
            Occ::Cand { .. } | Occ::Incomplete { .. } | Occ::CheckErr { .. } => true,
            _ => false,
        };
        if needs_values {
            let roots = self
                .codec
                .decode(&mut m.heap, &bytes[input.at..], MAX_RECORD_BYTES)
                .map_err(|error| {
                    if error.kind() == ErrorKind::Internal {
                        self.failure(error.to_string())
                    } else {
                        error
                    }
                })?;
            let count = m.s1.len();
            let state = roots
                .get(..count)
                .ok_or_else(|| self.failure("candidate state width".into()))?;
            m.s1.copy_from_slice(state);
            if let Occ::Cand { identity, .. } = &mut occ {
                if roots.len() != count + self.identity_width {
                    return Err(self.failure("candidate identity width".into()));
                }
                identity.extend_from_slice(&roots[count..]);
            } else if roots.len() != count {
                return Err(self.failure("noncandidate identity suffix".into()));
            }
        } else if input.at != bytes.len() {
            return Err(self.failure("unexpected occurrence payload".into()));
        }
        Ok((action, occ))
    }
    fn refresh(&mut self) {
        let stats = self.queue.stats();
        let mut usage = self.account.lock().unwrap_or_else(PoisonError::into_inner);
        usage.retained_bytes = usage
            .retained_bytes
            .saturating_sub(self.retained)
            .saturating_add(stats.retained_bytes);
        usage.peak_retained_bytes = usage.peak_retained_bytes.max(usage.retained_bytes);
        usage.spilled_bytes = usage
            .spilled_bytes
            .saturating_add(stats.spilled_bytes.saturating_sub(self.written));
        self.retained = stats.retained_bytes;
        self.written = stats.spilled_bytes;
    }
    fn failure(&self, message: String) -> Error {
        self.account
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failures
            .push(message.clone());
        Error::internal(message)
    }
}

impl Drop for Spool {
    fn drop(&mut self) {
        // Every exit, including an earlier source's deciding event, closes these
        // owned files. Failures enter the run owner and invalidate certification.
        let result = self.queue.close();
        self.refresh();
        if let Err(error) = result {
            let _ = self.failure(error.to_string());
        }
    }
}

struct Write {
    bytes: Vec<u8>,
}
impl Write {
    fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|&n| n <= MAX_RECORD_BYTES)
            .ok_or_else(|| Error::unsupported("candidate occurrence exceeds its record limit"))?;
        self.bytes
            .try_reserve_exact(end - self.bytes.len())
            .map_err(|_| Error::unsupported("candidate occurrence allocation"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.put(&[n])
    }
    fn word(&mut self, n: u64) -> Result<(), Error> {
        self.put(&n.to_le_bytes())
    }
    fn inv(&mut self, inv: &Inv) -> Result<(), Error> {
        match inv {
            Inv::Holds => self.byte(0),
            Inv::Violated(index) => {
                self.byte(1)?;
                self.word(*index as u64)
            }
            Inv::Error(index, error) => {
                self.byte(2)?;
                self.word(*index as u64)?;
                self.error(error)
            }
        }
    }
    fn error(&mut self, error: &Error) -> Result<(), Error> {
        self.byte(match error.kind() {
            ErrorKind::Unsupported => 0,
            ErrorKind::Eval => 1,
            ErrorKind::Internal => 2,
            ErrorKind::Stop => 3,
        })?;
        if let Some(tlc) = error.tlc() {
            self.byte(1)?;
            self.word(u64::from(tlc.code))?;
            self.byte(u8::from(tlc.eval_exception))?;
        } else {
            self.byte(0)?;
        }
        let (_, message) = error.clone().into_parts();
        self.word(message.len() as u64)?;
        self.put(message.as_bytes())
    }
    fn site(&mut self, site: Site) -> Result<(), Error> {
        let (tag, index) = match site {
            Site::Init => (0, 0),
            Site::Next(n) => (1, n),
            Site::Constraint(n) => (2, n),
            Site::ActionConstraint(n) => (3, n),
            Site::Invariant(n) => (4, n),
            Site::ImpliedInit(n) => (5, n),
            Site::ImpliedAction(n) => (6, n),
            Site::Incomplete => (7, 0),
            Site::Fingerprint => (8, 0),
            Site::View => (9, 0),
        };
        self.byte(tag)?;
        self.word(index as u64)
    }
}

struct Read<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Read<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(len)
            .filter(|&n| n <= self.bytes.len())
            .ok_or_else(|| Error::internal("truncated candidate occurrence"))?;
        let value = &self.bytes[self.at..end];
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::internal("candidate Boolean tag")),
        }
    }
    fn word(&mut self) -> Result<u64, Error> {
        let mut n = [0; 8];
        n.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(n))
    }
    fn index(&mut self) -> Result<usize, Error> {
        usize::try_from(self.word()?).map_err(|_| Error::internal("candidate index overflow"))
    }
    fn inv(&mut self) -> Result<Inv, Error> {
        match self.byte()? {
            0 => Ok(Inv::Holds),
            1 => Ok(Inv::Violated(self.index()?)),
            2 => {
                let n = self.index()?;
                Ok(Inv::Error(n, self.error()?))
            }
            _ => Err(Error::internal("candidate invariant tag")),
        }
    }
    fn error(&mut self) -> Result<Error, Error> {
        let kind = self.byte()?;
        let tlc = if self.boolean()? {
            Some(TlcClass {
                code: u32::try_from(self.word()?)
                    .map_err(|_| Error::internal("candidate error class"))?,
                eval_exception: self.boolean()?,
            })
        } else {
            None
        };
        let len = self.index()?;
        let message = std::str::from_utf8(self.take(len)?)
            .map_err(|_| Error::internal("candidate error message encoding"))?;
        match kind {
            0 if tlc.is_none() => Ok(Error::unsupported(message)),
            1 => Ok(tlc.map_or_else(
                || Error::eval(message),
                |tlc| Error::eval_class(tlc, message),
            )),
            2 if tlc.is_none() => Ok(Error::internal(message)),
            3 if tlc.is_none() && message.is_empty() => Ok(Error::stop()),
            _ => Err(Error::internal("candidate error kind")),
        }
    }
    fn site(&mut self) -> Result<Site, Error> {
        let tag = self.byte()?;
        let n = self.index()?;
        match (tag, n) {
            (0, 0) => Ok(Site::Init),
            (1, n) => Ok(Site::Next(n)),
            (2, n) => Ok(Site::Constraint(n)),
            (3, n) => Ok(Site::ActionConstraint(n)),
            (4, n) => Ok(Site::Invariant(n)),
            (5, n) => Ok(Site::ImpliedInit(n)),
            (6, n) => Ok(Site::ImpliedAction(n)),
            (7, 0) => Ok(Site::Incomplete),
            (8, 0) => Ok(Site::Fingerprint),
            (9, 0) => Ok(Site::View),
            _ => Err(Error::internal("candidate check site")),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "real candidate-file fault injection controls"
)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn candidate_checksum_and_unconsumed_queue_cleanup_failures_reach_owner() {
        use std::io::{Read as _, Seek, SeekFrom, Write as _};
        for cleanup in [false, true] {
            let parent = std::env::temp_dir().join(format!(
                "dive-candidate-fault-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&parent).unwrap();
            let store = Arc::new(dive_core::Store::new(true));
            let mut m = Machine::new(
                store,
                1,
                dive_core::Opts::default(),
                dive_core::memo::Memo::new(&[]),
            );
            let account = Arc::new(Mutex::new(CandidateUsage::default()));
            let mut spool = Spool::new(&m, 0, &parent, Arc::clone(&account), 0).unwrap();
            spool.keep(&mut m, &[Handle::TRUE]).unwrap();
            spool.push(0, &Occ::Incomplete { at: 0 }).unwrap();
            let owned = spool.queue.disk_directory().unwrap().to_path_buf();
            if cleanup {
                let moved = parent.join("held-files");
                std::fs::rename(&owned, &moved).unwrap();
                std::fs::write(&owned, b"cleanup obstruction").unwrap();
                drop(spool); // No ordered read consumed the queued suffix.
                assert!(
                    account
                        .lock()
                        .unwrap()
                        .failures
                        .iter()
                        .any(|m| m.contains("cleanup"))
                );
                std::fs::remove_file(owned).unwrap();
                std::fs::remove_dir_all(moved).unwrap();
            } else {
                let mut file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(owned.join("states"))
                    .unwrap();
                file.seek(SeekFrom::End(-1)).unwrap();
                let mut byte = [0];
                file.read_exact(&mut byte).unwrap();
                file.seek(SeekFrom::End(-1)).unwrap();
                file.write_all(&[byte[0] ^ 1]).unwrap();
                assert!(spool.read(0, &mut m).is_err());
                assert!(
                    account
                        .lock()
                        .unwrap()
                        .failures
                        .iter()
                        .any(|m| m.contains("checksum"))
                );
                drop(spool);
                assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
            }
            std::fs::remove_dir(parent).unwrap();
        }
    }
}
