//! The iteration order of a Java `java.util.Hashtable`, which SANY's contexts
//! are (`tla2sany/semantic/Context.java`, field `table`).
//!
//! SANY lists a module's `CONSTANT` and `VARIABLE` declarations by enumerating
//! the module context's `Hashtable` and reversing the result
//! (`ModuleNode.getConstantDecls`, `getVariableDecls`). TLC numbers the state
//! variables in that order (`SpecProcessor.processSpec` sets each name's
//! `varLoc`), and `TLCStateMut.fingerPrint` walks the values in it. So the order
//! depends on the table's layout: its capacity, every key's hash, and the order
//! in which the keys were inserted relative to each resize.
//!
//! This is a port of the parts of the JDK 17 `Hashtable` that decide the layout,
//! for a table built with the default constructor and only ever added to (SANY
//! never removes from a context, and a `put` of a present key only replaces its
//! value, in place):
//!
//! - `Hashtable()`: capacity 11, load factor `0.75f`, threshold 8;
//! - `addEntry`: when `count >= threshold`, `rehash()` first; the new entry goes
//!   at the *head* of bucket `(hash & 0x7FFFFFFF) % capacity`;
//! - `rehash()`: capacity `2 * old + 1`, threshold `(int) (capacity * 0.75f)`;
//!   old buckets are walked from the last to the first, each chain from its head,
//!   and every entry is moved to the head of its new bucket;
//! - `elements()` (`Hashtable.Enumerator`): buckets from the last to the first,
//!   each chain from its head.
//!
//! Keys hash as `String.hashCode` of their name: `UniqueString.hashCode` and
//! `SymbolTable.ModuleName.hashCode` both return it. Intern tokens play no part.
//!
//! **What the relative order of two keys depends on.** Insertion puts a key at
//! the head of its chain, and a resize moves entries one by one, so where two
//! keys end up relative to each other depends only on their own hashes, their
//! positions in the insertion sequence, and when the resizes happen, which
//! the number of keys inserted so far decides. The other keys matter only
//! through that count. In particular, the order of the built-in operators at
//! the start of every module context (which SANY's `Context.duplicate` inserts
//! newest first) cannot change the order of the declarations.

/// `java.lang.String.hashCode`: `s[0]*31^(n-1) + ... + s[n-1]` over the UTF-16
/// code units, in wrapping 32-bit arithmetic.
///
/// ```
/// use dive_sema::hashtable::java_string_hash;
/// assert_eq!(java_string_hash(""), 0);
/// assert_eq!(java_string_hash("x"), 120);
/// assert_eq!(java_string_hash("Aa"), java_string_hash("BB"));
/// assert_eq!(java_string_hash("hello"), 99_162_322);
/// ```
#[must_use]
pub fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(i32::from(u)))
}

/// The default initial capacity of `Hashtable()`.
const INITIAL_CAPACITY: usize = 11;
/// The default load factor of `Hashtable()`.
const LOAD_FACTOR: f32 = 0.75;

/// `(int) Math.min(capacity * loadFactor, MAX_ARRAY_SIZE + 1)`, with Java's
/// `float` arithmetic.
fn threshold(capacity: usize) -> usize {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Java's (int) of a float product, exact for any table SANY builds"
    )]
    let t = (capacity as f32 * LOAD_FACTOR) as usize;
    t
}

/// The bucket of a hash: `(hash & 0x7FFFFFFF) % capacity`.
fn bucket(hash: i32, capacity: usize) -> usize {
    usize::try_from(hash & 0x7FFF_FFFF).unwrap_or(0) % capacity
}

/// The order in which `Hashtable.elements()` enumerates a table built by
/// `new Hashtable<>()` followed by a `put` of each key in `hashes` (the keys'
/// `hashCode`s, all keys distinct), as indices into `hashes`.
///
/// ```
/// use dive_sema::hashtable::enumeration_order;
/// // Capacity 11: buckets 10, 4, 1 are enumerated from the last to the first.
/// assert_eq!(enumeration_order(&[1, 4, 10]), vec![2, 1, 0]);
/// // Colliding keys: the later one is at the head of the chain.
/// assert_eq!(enumeration_order(&[3, 14]), vec![1, 0]);
/// ```
#[must_use]
pub fn enumeration_order(hashes: &[i32]) -> Vec<usize> {
    // `table[b]` is the head of bucket `b`; `next[i]` follows entry `i`.
    let mut table: Vec<Option<usize>> = vec![None; INITIAL_CAPACITY];
    let mut next: Vec<Option<usize>> = Vec::with_capacity(hashes.len());
    let mut threshold_now = threshold(INITIAL_CAPACITY);
    for (i, &h) in hashes.iter().enumerate() {
        if i >= threshold_now {
            // `rehash()`.
            let capacity = table.len() * 2 + 1;
            threshold_now = threshold(capacity);
            let mut fresh: Vec<Option<usize>> = vec![None; capacity];
            for b in (0..table.len()).rev() {
                let mut e = table.get(b).copied().flatten();
                while let Some(cur) = e {
                    e = next.get(cur).copied().flatten();
                    let nb = hashes.get(cur).map_or(0, |&hc| bucket(hc, capacity));
                    if let (Some(slot), Some(link)) = (fresh.get_mut(nb), next.get_mut(cur)) {
                        *link = *slot;
                        *slot = Some(cur);
                    }
                }
            }
            table = fresh;
        }
        // `addEntry`: insert at the head of the bucket.
        let b = bucket(h, table.len());
        let head = table.get(b).copied().flatten();
        next.push(head);
        if let Some(slot) = table.get_mut(b) {
            *slot = Some(i);
        }
    }
    let mut out = Vec::with_capacity(hashes.len());
    for b in (0..table.len()).rev() {
        let mut e = table.get(b).copied().flatten();
        while let Some(cur) = e {
            out.push(cur);
            e = next.get(cur).copied().flatten();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{enumeration_order, java_string_hash, threshold};

    #[test]
    fn thresholds_follow_java() {
        // 11 -> 23 -> 47 -> 95 -> 191 -> 383 -> 767 -> 1535.
        let caps = [11, 23, 47, 95, 191, 383, 767, 1535];
        let ths = [8, 17, 35, 71, 143, 287, 575, 1151];
        for (c, t) in caps.iter().zip(ths) {
            assert_eq!(threshold(*c), t);
        }
    }

    #[test]
    fn rehash_moves_chains_in_java_order() {
        // Nine keys force one rehash (threshold 8) before the ninth insert.
        let hashes = [0, 23, 1, 2, 3, 4, 5, 6, 7];
        // At capacity 11, bucket 0 is [0]; bucket 1 is [23, 1] (23 % 11 = 1,
        // inserted first, then 1 at the head): chain 1 -> 23. Rehash walks
        // buckets 10..0: bucket 7..2 first, then bucket 1 (1, then 23), then
        // bucket 0 (0). At capacity 23: 23 and 0 share bucket 0; 23 is moved
        // first and 0 goes to the head: chain 0 -> 23.
        let order = enumeration_order(&hashes);
        let as_hashes: Vec<i32> = order.iter().map(|&i| hashes[i]).collect();
        assert_eq!(as_hashes, vec![7, 6, 5, 4, 3, 2, 1, 0, 23]);
    }

    #[test]
    fn negative_hashes_use_the_low_31_bits() {
        let h = java_string_hash("polygenelubricants");
        assert_eq!(h, i32::MIN);
        assert_eq!(enumeration_order(&[h, 1]), vec![1, 0]);
    }
}
