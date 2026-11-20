//! The host side of the R2 procedure (`docs/developer/performance.md`): the
//! clean environment, the NUMA binding, the CPU and storage controls, the
//! stock JRE CDS archive, the warm page cache and the harness-overhead probe.
//!
//! None of these is faked. A control the host does not expose (macOS has no
//! `numactl` and no cpufreq files) is recorded as absent, and the record
//! lists it as a reason the run is triage.
//!
//! ```
//! use dive_conformance::r2::host::check_environment;
//!
//! let clean = [("PATH", "/usr/bin"), ("HOME", "/home/bench")];
//! assert!(check_environment(clean.map(|(k, v)| (k.into(), v.into()))).is_ok());
//! let preload = [("LD_PRELOAD", "/tmp/x.so")];
//! assert!(check_environment(preload.map(|(k, v)| (k.into(), v.into()))).is_err());
//! ```

use crate::error::{Error, Result};
use crate::hash::sha256_file;
use crate::util::capture;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Variables a timed run's environment may hold (`env -i` with these).
pub const ALLOWED_ENV: &[&str] = &["PATH", "HOME", "DIVE_CACHE_DIR"];

/// Whether `name` changes what a timed checker does, so a timed run refuses
/// it: `DIVE_*` (other than `DIVE_CACHE_DIR`), allocator tuning, the
/// dynamic loader's preload and search path, and their macOS equivalents.
#[must_use]
pub fn refused_env(name: &str) -> bool {
    (name.starts_with("DIVE_") && name != "DIVE_CACHE_DIR")
        || name.starts_with("MALLOC_")
        || name.starts_with("MIMALLOC_")
        || name.starts_with("DYLD_")
        || matches!(name, "GLIBC_TUNABLES" | "LD_PRELOAD" | "LD_LIBRARY_PATH")
}

/// The environment a timed run started in, by variable name only (values
/// are never recorded).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    /// Every variable name, sorted.
    pub names: Vec<String>,
    /// Names outside [`ALLOWED_ENV`]: the run was not started under
    /// `env -i`, so it is triage.
    pub extra: Vec<String>,
}

/// Checks a timed run's environment: refuses any [`refused_env`] variable
/// and records the rest by name.
///
/// # Errors
/// Fails, naming them, if any refused variable is set.
pub fn check_environment(
    vars: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> Result<Environment> {
    let names: BTreeSet<String> = vars
        .into_iter()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    let refused: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| refused_env(n))
        .collect();
    if !refused.is_empty() {
        return Err(Error::Usage(format!(
            "timed R2 runs refuse {}: start the harness under `env -i PATH=... HOME=... DIVE_CACHE_DIR=...`",
            refused.join(", ")
        )));
    }
    let extra = names
        .iter()
        .filter(|n| !ALLOWED_ENV.contains(&n.as_str()))
        .cloned()
        .collect();
    Ok(Environment {
        names: names.into_iter().collect(),
        extra,
    })
}

/// The CPU and memory binding the harness (and so every checker it starts)
/// runs under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    /// `numactl --cpunodebind=0 --membind=0` is in force.
    pub applied: bool,
    /// What is in force, or why nothing is.
    pub description: String,
}

/// The policy fields of `numactl --show`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NumaPolicy {
    /// `policy:`.
    pub policy: String,
    /// `cpubind:` (or `nodebind:`).
    pub cpubind: String,
    /// `membind:`.
    pub membind: String,
}

impl NumaPolicy {
    /// Parses `numactl --show`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let v = v.trim().to_owned();
            match k.trim() {
                "policy" => p.policy = v,
                "cpubind" => p.cpubind = v,
                "nodebind" if p.cpubind.is_empty() => p.cpubind = v,
                "membind" => p.membind = v,
                _ => {}
            }
        }
        p
    }

    /// Whether this is `--cpunodebind=0 --membind=0`.
    #[must_use]
    pub fn is_node0(&self) -> bool {
        self.policy == "bind" && self.cpubind == "0" && self.membind == "0"
    }
}

/// The flag the harness adds when it re-executes itself under `numactl`, so
/// that a binding which still does not show is refused instead of looping.
pub const REEXEC_FLAG: &str = "--r2-bound";

/// Ensures the harness runs under `numactl --cpunodebind=0 --membind=0`.
///
/// On Linux with `numactl` on `PATH`, an unbound harness re-executes itself
/// under it (`args` are its own arguments, without the program) and does
/// not return. Without `numactl`, on other systems, or with `disabled`, the
/// binding is recorded as absent.
///
/// # Errors
/// Fails if the re-executed harness is still not bound, or `exec` fails.
pub fn ensure_binding(disabled: bool, reexeced: bool, args: &[String]) -> Result<Binding> {
    let absent = |why: &str| {
        Ok(Binding {
            applied: false,
            description: format!("absent: {why}"),
        })
    };
    if disabled {
        return absent("--no-bind");
    }
    if !cfg!(target_os = "linux") {
        return absent(&format!("numactl is Linux-only ({})", std::env::consts::OS));
    }
    let show = match capture(Command::new("numactl").arg("--show")) {
        Ok(show) => show,
        Err(e) => return absent(&format!("`numactl --show` failed: {e}")),
    };
    let policy = NumaPolicy::parse(&show);
    if policy.is_node0() {
        let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        return Ok(Binding {
            applied: true,
            description: format!("numactl --cpunodebind=0 --membind=0 ({cpus} CPUs)"),
        });
    }
    if reexeced {
        return Err(Error::Command(format!(
            "re-executed under numactl but `numactl --show` reports policy `{}`, cpubind `{}`, membind `{}`",
            policy.policy, policy.cpubind, policy.membind
        )));
    }
    reexec_bound(args)
}

#[cfg(target_os = "linux")]
fn reexec_bound(args: &[String]) -> Result<Binding> {
    use std::os::unix::process::CommandExt as _;
    let exe = std::env::current_exe()
        .map_err(|e| Error::Command(format!("cannot locate the harness binary: {e}")))?;
    let err = Command::new("numactl")
        .args(["--cpunodebind=0", "--membind=0"])
        .arg(exe)
        .args(args)
        .arg(REEXEC_FLAG)
        .exec();
    Err(Error::Command(format!(
        "cannot re-execute under numactl: {err}"
    )))
}

#[cfg(not(target_os = "linux"))]
fn reexec_bound(_args: &[String]) -> Result<Binding> {
    Err(Error::Command(
        "numactl re-execution is Linux-only".to_owned(),
    ))
}

/// Host facts that decide whether timings are comparable, read from the
/// system. `None` means the host does not expose the fact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostFacts {
    /// The DMI product name (the instance type on AWS Nitro).
    pub instance: Option<String>,
    /// The CPU model.
    pub cpu: Option<String>,
    /// The OS release.
    pub os: Option<String>,
    /// `uname -sr`.
    pub kernel: Option<String>,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// Logical CPUs this process may use.
    pub cpus: usize,
    /// Physical memory in bytes.
    pub memory_bytes: Option<u64>,
    /// SMT control (`off`, `on`, `notsupported`, ...).
    pub smt: Option<String>,
    /// `off` or `on`.
    pub turbo: Option<String>,
    /// The cpufreq governor (`mixed: ...` when CPUs differ).
    pub governor: Option<String>,
    /// Minimum frequency, kHz (`mixed` CPUs record `None`).
    pub min_khz: Option<u64>,
    /// Maximum frequency, kHz.
    pub max_khz: Option<u64>,
    /// Transparent huge pages mode.
    pub thp: Option<String>,
    /// The device holding the cache directory (`df -P`).
    pub storage: Option<String>,
}

fn read_trim(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The selected value of a sysfs choice list (`always [madvise] never`).
#[must_use]
pub fn bracketed(text: &str) -> Option<String> {
    let start = text.find('[')?;
    let end = text[start..].find(']')? + start;
    Some(text[start + 1..end].to_owned())
}

/// One value if every entry agrees, else `mixed: a, b`.
fn uniform(values: &BTreeSet<String>) -> Option<String> {
    match values.len() {
        0 => None,
        1 => values.iter().next().cloned(),
        _ => Some(format!(
            "mixed: {}",
            values.iter().cloned().collect::<Vec<_>>().join(", ")
        )),
    }
}

impl HostFacts {
    /// Reads the facts of this host; `cache_dir` locates the storage.
    #[must_use]
    pub fn probe(cache_dir: &Path) -> Self {
        let host = crate::report::Host::current();
        let mut f = Self {
            kernel: host.kernel,
            arch: host.arch,
            cpus: host.cpus,
            memory_bytes: host.memory_bytes,
            storage: capture(Command::new("df").arg("-P").arg(cache_dir))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .nth(1)
                        .and_then(|l| l.split_whitespace().next())
                        .map(str::to_owned)
                }),
            ..Self::default()
        };
        if cfg!(target_os = "macos") {
            let sysctl = |k: &str| {
                capture(Command::new("/usr/sbin/sysctl").args(["-n", k]))
                    .ok()
                    .map(|s| s.trim().to_owned())
            };
            f.instance = sysctl("hw.model");
            f.cpu = sysctl("machdep.cpu.brand_string");
            f.os = capture(Command::new("/usr/bin/sw_vers").arg("-productVersion"))
                .ok()
                .map(|v| format!("macOS {}", v.trim()));
            return f;
        }
        f.instance = read_trim("/sys/devices/virtual/dmi/id/product_name");
        f.cpu = std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|c| {
            c.lines()
                .find_map(|l| l.strip_prefix("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_owned())
        });
        f.os = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                    .map(|v| v.trim_matches('"').to_owned())
            });
        f.smt = read_trim("/sys/devices/system/cpu/smt/control");
        f.turbo = read_trim("/sys/devices/system/cpu/intel_pstate/no_turbo")
            .map(|v| if v == "1" { "off" } else { "on" }.to_owned())
            .or_else(|| {
                read_trim("/sys/devices/system/cpu/cpufreq/boost")
                    .map(|v| if v == "0" { "off" } else { "on" }.to_owned())
            });
        let (mut gov, mut min, mut max) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        if let Ok(dir) = std::fs::read_dir("/sys/devices/system/cpu") {
            for e in dir.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let is_cpu = name
                    .strip_prefix("cpu")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
                if !is_cpu {
                    continue;
                }
                let freq = e.path().join("cpufreq");
                gov.extend(read_trim(freq.join("scaling_governor")));
                min.extend(read_trim(freq.join("scaling_min_freq")));
                max.extend(read_trim(freq.join("scaling_max_freq")));
            }
        }
        f.governor = uniform(&gov);
        let khz = |s: &BTreeSet<String>| match s.len() {
            1 => s.iter().next().and_then(|v| v.parse().ok()),
            _ => None,
        };
        f.min_khz = khz(&min);
        f.max_khz = khz(&max);
        f.thp =
            read_trim("/sys/kernel/mm/transparent_hugepage/enabled").and_then(|t| bracketed(&t));
        f
    }

    /// Reasons these facts fall short of the reference controls (SMT off,
    /// turbo off, the performance governor, minimum = maximum frequency).
    #[must_use]
    pub fn shortfalls(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self.smt.as_deref() {
            Some("off" | "forceoff" | "notsupported" | "notimplemented") => {}
            Some(v) => out.push(format!("SMT is {v}, not off")),
            None => out.push("SMT state not observable".to_owned()),
        }
        match self.turbo.as_deref() {
            Some("off") => {}
            Some(v) => out.push(format!("turbo is {v}, not off")),
            None => out.push("turbo state not observable".to_owned()),
        }
        match self.governor.as_deref() {
            Some("performance") => {}
            Some(v) => out.push(format!("governor is {v}, not performance")),
            None => out.push("cpufreq governor not observable".to_owned()),
        }
        match (self.min_khz, self.max_khz) {
            (Some(a), Some(b)) if a == b => {}
            (Some(a), Some(b)) => {
                out.push(format!("frequency is not fixed: min {a} kHz, max {b} kHz"));
            }
            _ => out.push("CPU frequency limits not observable".to_owned()),
        }
        out
    }
}

/// One CDS archive of the JRE tree, matched against the pinned archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdsArchive {
    /// Path inside the JRE.
    pub path: String,
    /// SHA-256, the same in the tree and the pinned archive.
    pub sha256: String,
}

fn is_jsa(entry: &str) -> bool {
    Path::new(entry)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jsa"))
}

fn rel(entry: &str) -> String {
    entry.trim_start_matches("./").to_owned()
}

/// JVM flags that select or create a CDS archive other than the stock one.
#[must_use]
pub fn non_stock_cds_flag(args: &[String]) -> Option<&str> {
    args.iter().map(String::as_str).find(|a| {
        a.starts_with("-Xshare")
            || a.contains("SharedArchiveFile")
            || a.contains("ArchiveClassesAtExit")
            || a.contains("AutoCreateSharedArchive")
            || a.contains("SharedClassListFile")
    })
}

/// Checks that the unpacked JRE at `jre_dir` holds exactly the stock CDS
/// archives of the pinned `archive` (a `.tar.gz`), byte for byte: no
/// `AppCDS`, dynamic or regenerated archive.
///
/// # Errors
/// Fails if `tar` fails, an archive is missing or extra, or one differs.
pub fn check_cds(archive: &Path, jre_dir: &Path, scratch: &Path) -> Result<Vec<CdsArchive>> {
    let listing = capture(Command::new("tar").arg("-tzf").arg(archive))?;
    let pinned: BTreeSet<String> = listing.lines().filter(|l| is_jsa(l)).map(rel).collect();
    let mut tree = BTreeSet::new();
    walk(jre_dir, &mut |p| {
        if is_jsa(&p.to_string_lossy())
            && let Ok(r) = p.strip_prefix(jre_dir)
        {
            tree.insert(r.to_string_lossy().into_owned());
        }
    })?;
    if pinned != tree {
        let extra: Vec<_> = tree.difference(&pinned).cloned().collect();
        let missing: Vec<_> = pinned.difference(&tree).cloned().collect();
        return Err(Error::Config(format!(
            "the JRE's CDS archives differ from the pinned JRE: extra {extra:?}, missing {missing:?}"
        )));
    }
    if pinned.is_empty() {
        return Ok(Vec::new());
    }
    let entries: Vec<&str> = listing.lines().filter(|l| is_jsa(l)).collect();
    capture(
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(scratch)
            .args(&entries),
    )?;
    let mut out = Vec::new();
    for path in pinned {
        let stock = sha256_file(&scratch.join(&path))?;
        let unpacked = sha256_file(&jre_dir.join(&path))?;
        if stock != unpacked {
            return Err(Error::Config(format!(
                "the JRE's CDS archive {path} is not the pinned one (SHA-256 {unpacked}, pinned {stock})"
            )));
        }
        out.push(CdsArchive {
            path,
            sha256: stock,
        });
    }
    Ok(out)
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, &e))?;
    for e in entries {
        let e = e.map_err(|e| Error::io(dir, &e))?;
        let ty = e.file_type().map_err(|err| Error::io(&e.path(), &err))?;
        if ty.is_dir() {
            if e.file_name() != ".git" {
                walk(&e.path(), f)?;
            }
        } else if ty.is_file() {
            f(&e.path());
        }
    }
    Ok(())
}

/// What warming the page cache read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warmed {
    /// Files read.
    pub files: u64,
    /// Bytes read.
    pub bytes: u64,
    /// The inputs, as `$DIVE_CACHE_DIR`-relative or file names.
    pub inputs: Vec<String>,
}

/// Reads every regular file under `paths` (files or directories; `.git`
/// directories skipped), so both checkers start with their inputs in the
/// page cache.
///
/// # Errors
/// Fails if a file cannot be read.
pub fn warm(paths: &[PathBuf], cache_root: &Path) -> Result<Warmed> {
    use std::io::Read as _;
    let mut w = Warmed::default();
    let mut buf = vec![0_u8; 1 << 20];
    let mut first_err = None;
    let mut read = |p: &Path| {
        let r = std::fs::File::open(p).and_then(|mut f| {
            let mut n = 0_u64;
            loop {
                let k = f.read(&mut buf)?;
                if k == 0 {
                    break Ok(n);
                }
                n += k as u64;
            }
        });
        match r {
            Ok(n) => {
                w.files += 1;
                w.bytes += n;
            }
            Err(e) => {
                first_err.get_or_insert_with(|| Error::io(p, &e));
            }
        }
    };
    let mut inputs = Vec::new();
    for p in paths {
        inputs.push(p.strip_prefix(cache_root).map_or_else(
            |_| {
                p.file_name().map_or_else(
                    || p.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                )
            },
            |r| format!("$DIVE_CACHE_DIR/{}", r.display()),
        ));
        if p.is_dir() {
            walk(p, &mut read)?;
        } else {
            read(p);
        }
    }
    if let Some(e) = first_err {
        return Err(e);
    }
    w.inputs = inputs;
    Ok(w)
}

/// The fixed harness overhead: `/bin/true` through
/// [`crate::process::run_with_timeout`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverheadProbe {
    /// The program timed (`/bin/true`, or `/usr/bin/true` where there is
    /// no `/bin/true`, as on macOS).
    pub program: String,
    /// Each run, milliseconds.
    pub runs_ms: Vec<f64>,
    /// Their median.
    pub median_ms: f64,
}

/// Runs of the overhead probe.
pub const PROBE_RUNS: usize = 5;

/// Times `/bin/true` [`PROBE_RUNS`] times through the harness's process
/// runner, exactly as a checker run is timed.
///
/// # Errors
/// Fails if `/bin/true` cannot be run or does not exit 0.
pub fn overhead_probe(scratch: &Path) -> Result<OverheadProbe> {
    let program = ["/bin/true", "/usr/bin/true"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
        .ok_or_else(|| Error::Command("neither /bin/true nor /usr/bin/true exists".to_owned()))?;
    let mut runs_ms = Vec::new();
    for _ in 0..PROBE_RUNS {
        let o = crate::process::run_with_timeout(
            &mut Command::new(program),
            Duration::from_secs(30),
            scratch,
        )?;
        if o.exit_code != Some(0) {
            return Err(Error::Command(format!(
                "{program} exited {:?}",
                o.exit_code
            )));
        }
        runs_ms.push((o.elapsed.as_secs_f64() * 1e6).round() / 1e3);
    }
    let median_ms = super::stats::median(&runs_ms).unwrap_or_default();
    Ok(OverheadProbe {
        program: program.to_owned(),
        runs_ms,
        median_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(names: &[&str]) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        names.iter().map(|n| ((*n).into(), "x".into())).collect()
    }

    #[test]
    fn the_environment_refuses_tuning_and_records_extras_by_name() -> Result<()> {
        let env = check_environment(vars(&["PATH", "HOME", "DIVE_CACHE_DIR"]))?;
        assert_eq!(env.extra, Vec::<String>::new());
        assert_eq!(env.names, ["DIVE_CACHE_DIR", "HOME", "PATH"]);
        let env = check_environment(vars(&["PATH", "TERM", "GH_TOKEN"]))?;
        assert_eq!(env.extra, ["GH_TOKEN", "TERM"]);
        for bad in [
            "DIVE_ENGINE",
            "DIVE_COMPAT",
            "MALLOC_ARENA_MAX",
            "MIMALLOC_PAGE_RESET",
            "GLIBC_TUNABLES",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
        ] {
            let e = check_environment(vars(&["PATH", bad]));
            assert!(
                matches!(&e, Err(Error::Usage(m)) if m.contains(bad)),
                "{bad}: {e:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn numactl_show_is_parsed() {
        let bound = NumaPolicy::parse(
            "policy: bind\npreferred node: 0\nphyscpubind: 0 1 2\ncpubind: 0 \nnodebind: 0 \nmembind: 0 \n",
        );
        assert!(bound.is_node0(), "{bound:?}");
        let free = NumaPolicy::parse(
            "policy: default\npreferred node: current\ncpubind: 0 1 \nnodebind: 0 1 \nmembind: 0 1 \n",
        );
        assert!(!free.is_node0());
        assert!(!NumaPolicy::parse("").is_node0());
    }

    #[test]
    fn sysfs_choices_and_shortfalls() {
        assert_eq!(
            bracketed("always [madvise] never").as_deref(),
            Some("madvise")
        );
        assert_eq!(bracketed("none"), None);
        let reference = HostFacts {
            smt: Some("off".into()),
            turbo: Some("off".into()),
            governor: Some("performance".into()),
            min_khz: Some(2_400_000),
            max_khz: Some(2_400_000),
            ..HostFacts::default()
        };
        assert_eq!(reference.shortfalls(), Vec::<String>::new());
        let laptop = HostFacts::default();
        assert_eq!(laptop.shortfalls().len(), 4, "{:?}", laptop.shortfalls());
        let boosted = HostFacts {
            turbo: Some("on".into()),
            max_khz: Some(3_500_000),
            ..reference
        };
        let s = boosted.shortfalls();
        assert!(s.iter().any(|r| r.contains("turbo is on")), "{s:?}");
        assert!(s.iter().any(|r| r.contains("not fixed")), "{s:?}");
    }

    #[test]
    fn only_stock_cds_flags_pass() {
        let ok: Vec<String> = ["java", "-XX:+UseParallelGC", "-Xmx24g"]
            .map(String::from)
            .to_vec();
        assert_eq!(non_stock_cds_flag(&ok), None);
        for bad in [
            "-XX:SharedArchiveFile=app.jsa",
            "-XX:ArchiveClassesAtExit=x.jsa",
            "-XX:+AutoCreateSharedArchive",
            "-Xshare:off",
        ] {
            let mut v = ok.clone();
            v.push(bad.to_owned());
            assert_eq!(non_stock_cds_flag(&v), Some(bad));
        }
    }

    #[test]
    fn cds_archives_must_match_the_pinned_archive()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("cds")?;
        let src = dir.path().join("src");
        let jre = src.join("jre-1/lib/server");
        std::fs::create_dir_all(&jre)?;
        std::fs::write(jre.join("classes.jsa"), b"stock")?;
        std::fs::write(jre.join("libjvm.so"), b"jvm")?;
        let archive = dir.path().join("jre.tar.gz");
        capture(
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(&src)
                .arg("jre-1"),
        )?;
        let scratch = dir.path().join("scratch");
        std::fs::create_dir_all(&scratch)?;
        let ok = check_cds(&archive, &src, &scratch)?;
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].path, "jre-1/lib/server/classes.jsa");
        // A dynamic archive next to the stock one.
        std::fs::write(jre.join("app.jsa"), b"dynamic")?;
        let extra = check_cds(&archive, &src, &scratch);
        assert!(
            matches!(&extra, Err(Error::Config(m)) if m.contains("app.jsa")),
            "{extra:?}"
        );
        std::fs::remove_file(jre.join("app.jsa"))?;
        // A regenerated stock archive.
        std::fs::write(jre.join("classes.jsa"), b"regenerated")?;
        let changed = check_cds(&archive, &src, &scratch);
        assert!(
            matches!(&changed, Err(Error::Config(m)) if m.contains("not the pinned")),
            "{changed:?}"
        );
        Ok(())
    }

    #[test]
    fn warming_reads_every_input_but_git() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("warm")?;
        std::fs::create_dir_all(dir.path().join("c/.git"))?;
        std::fs::write(dir.path().join("c/a.tla"), b"abc")?;
        std::fs::write(dir.path().join("c/.git/pack"), b"zzzzzz")?;
        std::fs::write(dir.path().join("bin"), b"12345")?;
        let w = warm(&[dir.path().join("c"), dir.path().join("bin")], dir.path())?;
        assert_eq!((w.files, w.bytes), (2, 8));
        assert_eq!(w.inputs, ["$DIVE_CACHE_DIR/c", "$DIVE_CACHE_DIR/bin"]);
        Ok(())
    }

    #[test]
    fn the_overhead_probe_times_five_runs() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("probe")?;
        let p = overhead_probe(dir.path())?;
        assert_eq!(p.runs_ms.len(), PROBE_RUNS);
        assert!(p.median_ms > 0.0);
        Ok(())
    }
}
