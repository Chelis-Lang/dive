//! Standalone public-API probe for an already-built checkout.
use dive_explore::Seen;
use dive_store::predecessor::PredecessorLog;
use std::{env, error::Error};

fn key(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or("expected seen or parents")?;
    let count: u64 = args.next().ok_or("expected a count")?.parse()?;
    match mode.as_str() {
        "seen" => {
            let mut seen = Seen::default();
            for i in 0..count {
                assert!(seen.insert(key(i)));
            }
            for i in (0..count).step_by(997) {
                assert!(seen.contains(key(i)));
                assert!(!seen.insert(key(i)));
            }
            assert_eq!(seen.len() as u64, count);
        }
        "parents" if count > 0 => {
            let mut log = PredecessorLog::new(None, None);
            log.push_initial(0)?;
            for id in 1..count {
                log.push((id - 1) / 3, u32::try_from((id - 1) % 3)?)?;
            }
            for id in (1..count).step_by(7919) {
                let step = log.get(id)?;
                assert_eq!(step.parent, Some((id - 1) / 3));
                assert_eq!(u64::from(step.ordinal), (id - 1) % 3);
            }
            println!(
                "{} records, {} retained bytes",
                log.len(),
                log.usage().retained_bytes
            );
        }
        _ => return Err("expected seen or parents and a positive count".into()),
    }
    Ok(())
}
