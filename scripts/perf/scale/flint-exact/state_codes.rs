//! Exact codes for one compiled model per process. A new bit is allocated only
//! when a column needs it, so all prior rows keep their code. Refuse overflow.
use crate::value::{var_hash, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Eq, PartialEq, Ord, PartialOrd)]
enum Mathematical {
    Bool(bool),
    Int(i64),
    Str(u32),
    Model(u32),
    Set(Vec<Mathematical>),
    Function(Vec<(Mathematical, Mathematical)>),
}
fn normalize(v: &Value) -> Result<Mathematical, String> {
    Ok(match v {
        Value::Bool(v) => Mathematical::Bool(*v),
        Value::Int(v) => Mathematical::Int(*v),
        Value::Str(v) => Mathematical::Str(*v),
        Value::Model(v) => Mathematical::Model(*v),
        Value::Set(v) => {
            let mut a = v.iter().map(normalize).collect::<Result<Vec<_>, _>>()?;
            a.sort_unstable();
            a.dedup();
            Mathematical::Set(a)
        }
        Value::Seq(v) => Mathematical::Function(
            v.iter()
                .enumerate()
                .map(|(i, x)| {
                    Ok((
                        Mathematical::Int(i64::try_from(i + 1).map_err(|_| "sequence index")?),
                        normalize(x)?,
                    ))
                })
                .collect::<Result<_, String>>()?,
        ),
        Value::Func(v) => {
            if v.keys.len() != v.vals.len() {
                return Err("invalid function width".into());
            }
            let mut a = v
                .keys
                .iter()
                .zip(&v.vals)
                .map(|(k, x)| Ok((normalize(k)?, normalize(x)?)))
                .collect::<Result<Vec<_>, String>>()?;
            a.sort_unstable();
            if a.windows(2).any(|p| p[0].0 == p[1].0) {
                return Err("duplicate mathematical function key".into());
            }
            Mathematical::Function(a)
        }
        Value::Lazy(_) => {
            return Err("exact column codes require materialized finite values".into())
        }
    })
}

#[derive(Default)]
struct Column {
    words: BTreeMap<Mathematical, u64>,
    positions: Vec<usize>,
}
struct Registry {
    columns: Vec<Mutex<Column>>,
    next: [AtomicUsize; 2],
    factored: bool,
    halves: [Vec<Mutex<HashMap<u64, u32>>>; 2],
}
static REGISTRY: OnceLock<Registry> = OnceLock::new();
const SHARDS: usize = 256;
fn registry(width: usize) -> Result<&'static Registry, String> {
    let r = REGISTRY.get_or_init(|| Registry {
        columns: (0..width).map(|_| Mutex::default()).collect(),
        next: Default::default(),
        factored: std::env::var_os("FLINT_FACTORED_HALVES").is_some(),
        halves: std::array::from_fn(|_| (0..SHARDS).map(|_| Mutex::default()).collect()),
    });
    if r.columns.len() != width {
        return Err("state row width changed".into());
    }
    Ok(r)
}
thread_local! {
  static VALUES:RefCell<Vec<HashMap<u64,Vec<(Value,u64)>>>>=RefCell::default();
  static HALVES:RefCell<[HashMap<u64,u32>;2]>=RefCell::default();
}
fn mix(mut x: u64) -> u64 {
    // Both multipliers are odd. Each xor-shift is invertible and fixes zero.
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
// The upstream structural hash enumerates lazy sets. Reject them while hashing
// each concrete child, before a cache hit or comparison can observe that value.
fn cache_hash(value: &Value) -> Result<u64, String> {
    fn append(hash: u64, child: &Value) -> Result<u64, String> {
        Ok(hash.wrapping_mul(0x9e3779b185ebca87) ^ cache_hash(child)?)
    }
    match value {
        Value::Bool(_) | Value::Int(_) | Value::Str(_) | Value::Model(_) => Ok(var_hash(value)),
        Value::Set(items) => items.iter().try_fold(17u64, append),
        Value::Seq(items) => items.iter().try_fold(19u64, append),
        Value::Func(f) => {
            if f.keys.len() != f.vals.len() {
                return Err("invalid function width".into());
            }
            f.keys
                .iter()
                .zip(f.vals.iter())
                .try_fold(23u64, |h, (k, v)| append(append(h, k)?, v))
        }
        Value::Lazy(_) => Err("exact column codes require materialized finite values".into()),
    }
}
pub fn value(width: usize, i: usize, v: &Value) -> Result<u64, String> {
    let r = registry(width)?;
    // Raw hashes select cache buckets. Compare complete values on a hit.
    let h = cache_hash(v)?;
    VALUES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_empty() {
            cache.resize_with(width, HashMap::new)
        }
        let local = &mut cache[i];
        if let Some(code) = local.get(&h).and_then(|b| b.iter().find(|x| &x.0 == v)) {
            return Ok(code.1);
        }
        let canonical = normalize(v)?;
        let mut col = r.columns[i].lock().map_err(|_| "column lock")?;
        let code = if let Some(&word) = col.words.get(&canonical) {
            word
        } else {
            let id = col.words.len();
            let bits = (usize::BITS - id.leading_zeros()) as usize;
            if bits > col.positions.len() {
                let bank = if r.factored { i % 2 } else { 0 };
                let limit = if r.factored { 64 } else { 63 };
                let bit = r.next[bank]
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                        (n < limit).then_some(n + 1)
                    })
                    .map_err(|_| {
                        format!("exact state code exceeded {limit} bits in bank {bank}, column {i}")
                    })?;
                col.positions.push(bit);
            }
            let mut word = 0;
            for (j, &bit) in col.positions.iter().enumerate() {
                word |= (((id >> j) & 1) as u64) << bit
            }
            col.words.insert(canonical, word);
            word
        };
        drop(col);
        if local.len() >= 2048 {
            local.clear();
        }
        local.entry(h).or_default().push((v.clone(), code));
        Ok(code)
    })
}
fn half(r: &Registry, bank: usize, word: u64) -> Result<u32, String> {
    HALVES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let local = &mut cache[bank];
        if let Some(&id) = local.get(&word) {
            return Ok(id);
        }
        let shard = (mix(word) >> (64 - 8)) as usize;
        let mut table = r.halves[bank][shard].lock().map_err(|_| "half lock")?;
        let id = if let Some(&id) = table.get(&word) {
            id
        } else {
            let slot = table.len();
            // Reserve zero in the root encoding by reserving the top slot in every shard.
            if slot >= 0x00ff_ffff {
                return Err("exact half dictionary exceeded 24-bit shard capacity".into());
            }
            let id = ((shard as u32) << 24) | (slot as u32);
            table.insert(word, id);
            id
        };
        drop(table);
        if local.len() >= 16384 {
            local.clear()
        }
        local.insert(word, id);
        Ok(id)
    })
}
pub fn row(words: &[u64]) -> Result<u64, String> {
    let r = registry(words.len())?;
    let key = if r.factored {
        let mut pair = [0; 2];
        for (i, x) in words.iter().enumerate() {
            pair[i % 2] ^= x;
        }
        (u64::from(half(r, 0, pair[0])?) << 32) | u64::from(half(r, 1, pair[1])?)
    } else {
        words.iter().fold(0, |acc, x| acc ^ x)
    };
    // The compiler folds zero to one. A nonzero permutation avoids that alias.
    Ok(mix(key
        .checked_add(1)
        .ok_or("exhausted exact nonzero keys")?))
}
pub fn state(st: &[Value]) -> Result<u64, String> {
    row(&st
        .iter()
        .enumerate()
        .map(|(i, v)| value(st.len(), i, v))
        .collect::<Result<Vec<_>, _>>()?)
}
pub fn describe() {
    if let Some(r) = REGISTRY.get() {
        let cols: Vec<_> = r
            .columns
            .iter()
            .filter_map(|c| c.lock().ok().map(|x| (x.words.len(), x.positions.len())))
            .collect();
        eprintln!("EXACT_COLUMNS {cols:?}");
        eprintln!(
            "EXACT_BANK_BITS {} {}",
            r.next[0].load(Ordering::Relaxed),
            r.next[1].load(Ordering::Relaxed)
        );
    }
}

struct Audit {
    mask: u64,
    fingerprints: crate::store::FpSet,
    excess: std::sync::atomic::AtomicU64,
}
static AUDIT: OnceLock<Result<Option<Audit>, String>> = OnceLock::new();

/// Compare the original fingerprint with every first exact admission.
/// A repeat counts a pair of unequal mathematical states sharing a folded key.
pub fn audit(state: &[Value], exact_key: u64) -> Result<(), String> {
    let audit = AUDIT
        .get_or_init(|| {
            let Some(mode) = std::env::var_os("FLINT_AUDIT_FP_BITS") else {
                return Ok(None);
            };
            let width: u32 = mode
                .to_str()
                .ok_or("invalid fingerprint width")?
                .parse()
                .map_err(|_| "invalid fingerprint width")?;
            let mask = match width {
                64 => u64::MAX,
                0..=63 => (1u64 << width) - 1,
                _ => return Err("fingerprint width exceeds 64".into()),
            };
            Ok(Some(Audit {
                mask,
                fingerprints: Default::default(),
                excess: 0.into(),
            }))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    if let Some(audit) = audit {
        let fp = crate::value::fingerprint(state) & audit.mask;
        if !audit.fingerprints.insert(fp)? {
            let previous = audit.excess.fetch_add(1, Ordering::Relaxed);
            if previous < 16 {
                eprintln!("COMPILED_FP_REPEAT {fp:016x} {exact_key:016x}");
            }
        }
    }
    Ok(())
}
pub fn describe_audit() {
    if let Some(Ok(Some(a))) = AUDIT.get() {
        eprintln!("COMPILED_FP_EXCESS {}", a.excess.load(Ordering::Relaxed));
    }
}
