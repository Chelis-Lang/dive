//! Bounded mathematical keys for closed canonical state rows.
//!
//! Keys contain values, never Store coordinates or construction authority. This
//! read-only traversal cannot normalize an original receiver or invoke a provider.

use crate::handle::Tag;
use crate::node::{Kind, flags};
use crate::{Handle, Heap};

/// Why a canonical-key traversal did not complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyError {
    /// The key would exceed its explicit byte/workspace policy.
    Limit(String),
    /// A value has no admitted closed mathematical encoding.
    Unsupported(String),
    /// The actual run owner cancelled the traversal.
    Interrupted,
    /// An invalid internal value was supplied.
    Internal(String),
}

enum Task {
    Value(Handle),
    Integer(i64),
}

struct Writer {
    bytes: Vec<u8>,
    limit: usize,
}

impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<(), KeyError> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.limit)
        {
            return Err(KeyError::Limit("canonical seen key byte limit".into()));
        }
        if self.bytes.capacity() < self.bytes.len() + bytes.len() {
            let target = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(self.bytes.len() + bytes.len())
                .min(self.limit);
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| KeyError::Limit("canonical seen key allocation".into()))?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn len(&mut self, len: usize) -> Result<(), KeyError> {
        self.put(&(len as u64).to_le_bytes())
    }

    fn integer(&mut self, bytes: &[u8]) -> Result<(), KeyError> {
        self.put(&[2])?;
        self.len(bytes.len())?;
        self.put(bytes)
    }
}

/// Encodes complete mathematical identities without converting to reference values.
///
/// The row must already be canonical and closed. Integer-domain functions,
/// general functions, records and tuples share one function encoding. String
/// and model-value identities use their text, independently of Store tokens.
/// Work and workspace are bounded by `max_bytes`, and the actual owner is
/// checked every 128 values. Original/history values are refused, not normalized.
///
/// ```
/// use std::sync::Arc;
/// use dive_core::{Handle, Heap, Store};
/// let heap = Heap::new(Arc::new(Store::default()), true, false);
/// let bytes = dive_core::key::canonical(&heap, &[Handle::TRUE], 1024, &|| true)?;
/// assert!(bytes.starts_with(b"DIVECK01"));
/// # Ok::<(), dive_core::key::KeyError>(())
/// ```
///
/// # Errors
/// [`KeyError`] distinguishes cancellation, explicit limits, unimplemented values
/// and invalid internal handles. No partial key is returned.
pub fn canonical(
    heap: &Heap,
    row: &[Handle],
    max_bytes: usize,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u8>, KeyError> {
    if !proceed() {
        return Err(KeyError::Interrupted);
    }
    let mut out = Writer {
        bytes: Vec::new(),
        limit: max_bytes,
    };
    out.put(b"DIVECK01")?;
    out.len(row.len())?;
    let mut stack = Vec::new();
    push(
        &mut stack,
        row.iter().rev().copied().map(Task::Value),
        max_bytes,
        proceed,
    )?;
    let mut steps = 0usize;
    while let Some(task) = stack.pop() {
        if steps.is_multiple_of(128) && !proceed() {
            return Err(KeyError::Interrupted);
        }
        steps = steps.saturating_add(1);
        let h = match task {
            Task::Value(h) => h,
            Task::Integer(n) => {
                let mut bytes = n.to_le_bytes().to_vec();
                while bytes.len() > 1 {
                    let last = bytes[bytes.len() - 1];
                    let next = bytes[bytes.len() - 2];
                    if (last == 0 && next & 128 == 0) || (last == 255 && next & 128 != 0) {
                        bytes.pop();
                    } else {
                        break;
                    }
                }
                out.integer(&bytes)?;
                continue;
            }
        };
        if heap.is_int(h) {
            let value = heap
                .big(h)
                .ok_or_else(|| KeyError::Internal("integer without magnitude".into()))?;
            out.integer(&value.to_signed_bytes_le())?;
            continue;
        }
        encode_value(heap, h, &mut out, &mut stack, max_bytes, proceed)?;
    }
    if heap.poisoned() {
        return Err(KeyError::Internal("poisoned canonical seen value".into()));
    }
    Ok(out.bytes)
}

fn encode_value(
    heap: &Heap,
    h: Handle,
    out: &mut Writer,
    stack: &mut Vec<Task>,
    max_bytes: usize,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<(), KeyError> {
    match h.tag() {
        Tag::Bool => out.put(&[1, u8::from(h == Handle::TRUE)])?,
        Tag::Str | Tag::Model => {
            let id = h
                .as_str_id()
                .or_else(|| h.as_model_id())
                .ok_or_else(|| KeyError::Internal("name without token".into()))?;
            let text = heap
                .store()
                .strings()
                .text(id)
                .ok_or_else(|| KeyError::Internal("unknown name token".into()))?;
            out.put(&[if h.as_str_id().is_some() { 3 } else { 4 }])?;
            out.len(text.len())?;
            out.put(text.as_bytes())?;
        }
        Tag::Persistent => {
            let v = heap.view(h);
            if v.flags & (flags::VIEW | flags::LAZY) != 0 {
                return Err(KeyError::Unsupported(
                    "noncanonical or deferred seen key".into(),
                ));
            }
            match v.kind {
                Kind::Set => {
                    out.put(&[5])?;
                    out.len(v.data.len())?;
                    push(
                        stack,
                        v.data.iter().rev().copied().map(Task::Value),
                        max_bytes,
                        proceed,
                    )?;
                }
                Kind::IntFcn => {
                    out.put(&[6])?;
                    out.len(v.data.len())?;
                    for (i, &value) in v.data.iter().enumerate().rev() {
                        let index = i64::try_from(i)
                            .ok()
                            .and_then(|i| v.lo.checked_add(i))
                            .ok_or_else(|| KeyError::Internal("integer domain overflow".into()))?;
                        push(
                            stack,
                            [Task::Value(value), Task::Integer(index)],
                            max_bytes,
                            proceed,
                        )?;
                    }
                }
                Kind::Fcn => {
                    if !v.data.len().is_multiple_of(2) {
                        return Err(KeyError::Internal("odd function data".into()));
                    }
                    let n = v.data.len() / 2;
                    out.put(&[6])?;
                    out.len(n)?;
                    for i in (0..n).rev() {
                        push(
                            stack,
                            [Task::Value(v.data[n + i]), Task::Value(v.data[i])],
                            max_bytes,
                            proceed,
                        )?;
                    }
                }
                _ => {
                    return Err(KeyError::Unsupported(format!(
                        "symbolic {:?} in disk seen key",
                        v.kind
                    )));
                }
            }
        }
        _ => {
            return Err(KeyError::Internal(
                "transient, empty or invalid canonical seen value".into(),
            ));
        }
    }
    Ok(())
}

fn push(
    stack: &mut Vec<Task>,
    tasks: impl IntoIterator<Item = Task>,
    max: usize,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<(), KeyError> {
    for task in tasks {
        if stack.len().is_multiple_of(128) && !proceed() {
            return Err(KeyError::Interrupted);
        }
        if stack
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(std::mem::size_of::<Task>()))
            .is_none_or(|n| n > max)
        {
            return Err(KeyError::Limit(
                "canonical seen key traversal workspace".into(),
            ));
        }
        if stack.capacity() == stack.len() {
            let cap = max / std::mem::size_of::<Task>();
            let target = stack
                .capacity()
                .saturating_mul(2)
                .max(stack.len() + 1)
                .min(cap);
            stack
                .try_reserve_exact(target - stack.len())
                .map_err(|_| KeyError::Limit("canonical seen key traversal allocation".into()))?;
        }
        stack.push(task);
    }
    Ok(())
}
