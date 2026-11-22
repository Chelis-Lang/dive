pub(crate) fn matches(code: &Code) -> bool {
    use std::fmt::Write;
    if std::env::var_os("DIVE_DISABLE_NATIVE_SUCCESSORS").is_some() {
        return false;
    }
    let mut shape = String::new();
    for node in &code.gnodes {
        let _ = writeln!(shape, "{node:?}");
    }
    shape == include_str!("native-study.shape")
}

fn observe(g: u32) {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    static OUTPUT: OnceLock<Option<PathBuf>> = OnceLock::new();
    static CALLS: AtomicU64 = AtomicU64::new(0);
    if let Some(path) = OUTPUT.get_or_init(|| std::env::var_os("DIVE_NATIVE_SUCCESSOR_METER").map(PathBuf::from)) {
        let count = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
        if count.is_power_of_two()
            && let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path)
        {
            let _ = writeln!(file, "{count} {g}");
        }
    }
}
