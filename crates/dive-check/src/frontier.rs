//! Active ordered safety frontiers, with an opt-in encoded/disk path.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use dive_eval::next::StateVals;
use dive_store::spill::{MAX_RECORD_BYTES, OrderedFrontier};
use dive_store::state::{StateDecoder, encode_state};
use dive_store::trace::StateId;
use dive_value::{Ctx, ErrorKind, Interner, codec};

use crate::{CheckOptions, Verdict};

pub use dive_runtime::usage::FrontierUsage;

#[derive(Debug)]
pub(crate) struct Accounting {
    pub usage: FrontierUsage,
}
pub(crate) type Shared = Arc<Mutex<Accounting>>;

pub(crate) fn accounting(opts: &CheckOptions) -> Shared {
    Arc::new(Mutex::new(Accounting {
        usage: FrontierUsage {
            memory_bytes_per_queue: opts.frontier_memory_bytes,
            max_record_bytes: opts.frontier_memory_bytes.map(|_| MAX_RECORD_BYTES),
            ..FrontierUsage::default()
        },
    }))
}

pub(crate) fn evidence(shared: &Shared) -> FrontierUsage {
    let mut usage = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .usage
        .clone();
    usage.failures.sort();
    usage.failures.dedup();
    usage
}

pub(crate) struct Queue {
    memory: Vec<(StateVals, StateId)>,
    encoded: Option<OrderedFrontier>,
    shared: Shared,
    ctx: Ctx,
    limit: Option<usize>,
    directory: PathBuf,
}

impl Queue {
    pub(crate) fn new(opts: &CheckOptions, shared: Shared) -> Self {
        let directory = opts
            .frontier_directory
            .clone()
            .unwrap_or_else(std::env::temp_dir);
        Self {
            memory: Vec::new(),
            encoded: opts
                .frontier_memory_bytes
                .map(|n| OrderedFrontier::new(n, &directory)),
            shared,
            ctx: opts.eval.ctx,
            limit: opts.frontier_memory_bytes,
            directory,
        }
    }
    pub(crate) fn empty_like(&self) -> Self {
        Self {
            memory: Vec::new(),
            encoded: self.limit.map(|n| OrderedFrontier::new(n, &self.directory)),
            shared: self.shared.clone(),
            ctx: self.ctx,
            limit: self.limit,
            directory: self.directory.clone(),
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.encoded
            .as_ref()
            .map_or(self.memory.len(), OrderedFrontier::len)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) fn read(
        &self,
        at: usize,
        names: &Interner,
    ) -> Result<(StateVals, StateId), Verdict> {
        let result = (|| {
            if let Some(encoded) = &self.encoded {
                let (id, bytes) = encoded.read(at).map_err(storage_error)?;
                let vals = StateDecoder::new(names).decode(&bytes).map_err(|e| {
                    Verdict::ResourceLimit {
                        message: format!("frontier decode: {e}"),
                    }
                })?;
                Ok((vals.into_iter().map(Some).collect::<Vec<_>>().into(), id))
            } else {
                self.memory
                    .get(at)
                    .cloned()
                    .ok_or_else(|| Verdict::InternalError {
                        message: "frontier position outside queue".into(),
                    })
            }
        })();
        if let Err(ref error) = result {
            self.shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .usage
                .failures
                .push(format!("{error:?}"));
        }
        result
    }
    pub(crate) fn push(&mut self, vals: StateVals, id: StateId) -> Result<(), Verdict> {
        let Some(encoded) = &mut self.encoded else {
            self.memory.push((vals, id));
            return Ok(());
        };
        let mut size = 0usize;
        let concrete = vals
            .iter()
            .map(|v| {
                v.clone().ok_or_else(|| Verdict::InternalError {
                    message: "incomplete state queued for frontier encoding".into(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for v in &concrete {
            let bound = codec::frontier_size_bound(v, MAX_RECORD_BYTES.saturating_sub(size))
                .map_err(|e| {
                    if e.kind() == ErrorKind::NotEnumerable {
                        Verdict::Unsupported {
                            feature: e.message().to_owned(),
                        }
                    } else {
                        Verdict::ResourceLimit {
                            message: e.message().to_owned(),
                        }
                    }
                })?;
            size = size.saturating_add(bound);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|e| Verdict::ResourceLimit {
                message: format!("frontier encode allocation: {e}"),
            })?;
        encode_state(&concrete, self.ctx, &mut bytes).map_err(|e| Verdict::ResourceLimit {
            message: format!("frontier encode: {e}"),
        })?;
        let before = encoded.stats();
        let result = encoded.push(id, &bytes).map_err(storage_error);
        let after = encoded.stats();
        let mut accounting = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        accounting.usage.retained_bytes = accounting
            .usage
            .retained_bytes
            .saturating_sub(before.retained_bytes)
            .saturating_add(after.retained_bytes);
        accounting.usage.peak_retained_bytes = accounting
            .usage
            .peak_retained_bytes
            .max(accounting.usage.retained_bytes);
        accounting.usage.spilled_bytes = accounting
            .usage
            .spilled_bytes
            .saturating_add(after.spilled_bytes.saturating_sub(before.spilled_bytes));
        if let Err(ref e) = result {
            accounting.usage.failures.push(format!("{e:?}"));
        }
        result
    }
}

pub(crate) fn storage_error(e: impl std::fmt::Display) -> Verdict {
    Verdict::ResourceLimit {
        message: e.to_string(),
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        if let Some(encoded) = &mut self.encoded {
            let before = encoded.stats();
            let result = encoded.close();
            let mut a = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
            a.usage.retained_bytes = a.usage.retained_bytes.saturating_sub(before.retained_bytes);
            if let Err(e) = result {
                a.usage.failures.push(e.to_string());
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "deterministic filesystem failure controls"
)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn prepared() -> (PathBuf, Shared, Queue) {
        let parent = std::env::temp_dir().join(format!(
            "dive-active-frontier-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&parent).expect("parent");
        let mut opts = CheckOptions::tlc(0, 0);
        opts.frontier_memory_bytes = Some(0);
        opts.frontier_directory = Some(parent.clone());
        let shared = accounting(&opts);
        let mut q = Queue::new(&opts, shared.clone());
        q.push(vec![Some(dive_value::Value::Int(1))].into(), 4)
            .expect("record");
        (parent, shared, q)
    }
    #[test]
    fn missing_disk_payload_is_recorded_as_no_verdict_storage_failure() {
        let (parent, shared, q) = prepared();
        let path = q
            .encoded
            .as_ref()
            .expect("encoded")
            .disk_directory()
            .expect("disk")
            .join("states");
        std::fs::remove_file(path).expect("remove payload");
        assert!(matches!(
            q.read(0, &Interner::new()),
            Err(Verdict::ResourceLimit { .. })
        ));
        drop(q);
        assert_ne!(evidence(&shared).failures, [] as [String; 0]);
        assert_eq!(evidence(&shared).retained_bytes, 0);
        std::fs::remove_dir(parent).expect("parent cleanup");
    }
    #[test]
    fn cleanup_failure_reaches_final_accounting_instead_of_disappearing_in_drop() {
        let (parent, shared, q) = prepared();
        let path = q
            .encoded
            .as_ref()
            .expect("encoded")
            .disk_directory()
            .expect("disk")
            .to_owned();
        std::fs::remove_dir_all(&path).expect("remove original directory");
        std::fs::write(&path, b"replacement is not a directory").expect("replace");
        drop(q);
        assert!(
            evidence(&shared)
                .failures
                .iter()
                .any(|s| s.contains("cleanup"))
        );
        std::fs::remove_file(path).expect("replacement cleanup");
        std::fs::remove_dir(parent).expect("parent cleanup");
    }
}
