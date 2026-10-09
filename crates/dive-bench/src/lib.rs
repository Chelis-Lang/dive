//! `dive-bench`: benchmarks against TLC baselines (M5 WP-G1). Run by hand,
//! never in the per-change CI gate.
//!
//! The `dive-bench` binary times `dive check` at one worker, in the
//! `--compat=tlc` and default profiles, on the fixed benchmark set in
//! `benchmarks.toml`. Every run is checked against the model's TLC golden
//! (verdict, distinct and generated states): a fast wrong answer is a
//! failure. It reports CPU time (user + system, which a busy machine
//! disturbs far less than wall time), wall time, distinct states per CPU
//! second and peak RSS, as medians over interleaved repetitions, and
//! compares them with TLC's single-worker time:
//!
//! - the golden's `elapsed_ms` (one TLC run while the goldens were made);
//! - the survey's measured medians (`docs/survey/data/bench-summary.csv`);
//! - with `--tlc`, TLC runs of the same configuration on this machine, timed
//!   the same way.
//!
//! This library holds the parts with no process handling: the benchmark
//! set, resource-usage parsing and the statistics.
//!
//! ```
//! let set = dive_bench::load_benchmarks(
//!     "[[bench]]\nname = \"G\"\nmodel = \"specifications/G/G.cfg\"\nwhy = \"x\"\n",
//! )
//! .expect("parses");
//! assert_eq!(set[0].name, "G");
//! assert_eq!(dive_bench::median(&[3.0, 1.0, 2.0]), Some(2.0));
//! ```
#![forbid(unsafe_code)]

use serde::Deserialize;

/// One benchmark: an Examples model.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bench {
    /// Short name, used by `--filter` and in reports.
    pub name: String,
    /// The model: the corpus-relative `.cfg` path (the golden's model id).
    pub model: String,
    /// The row of the survey's `bench-summary.csv`, if the survey ran it.
    #[serde(default)]
    pub survey: Option<String>,
    /// Why it is in the set.
    pub why: String,
    /// A replacement configuration (only to drop temporal properties).
    #[serde(default)]
    pub cfg: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchFile {
    bench: Vec<Bench>,
}

/// Parses `benchmarks.toml`.
///
/// # Errors
/// Fails on malformed TOML, an empty set or a duplicate name.
pub fn load_benchmarks(text: &str) -> Result<Vec<Bench>, String> {
    let file: BenchFile = toml::from_str(text).map_err(|e| e.to_string())?;
    if file.bench.is_empty() {
        return Err("the benchmark set is empty".to_owned());
    }
    let mut names = std::collections::BTreeSet::new();
    for b in &file.bench {
        if !names.insert(b.name.as_str()) {
            return Err(format!("duplicate benchmark `{}`", b.name));
        }
        if !std::path::Path::new(&b.model)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cfg"))
        {
            return Err(format!("benchmark `{}`: model must be a .cfg", b.name));
        }
    }
    Ok(file.bench)
}

/// A child process's resource usage, as `/usr/bin/time` reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Usage {
    /// User CPU seconds.
    pub user_s: f64,
    /// System CPU seconds.
    pub sys_s: f64,
    /// Peak resident set size in bytes.
    pub max_rss_bytes: u64,
}

impl Usage {
    /// User plus system CPU seconds.
    #[must_use]
    pub fn cpu_s(&self) -> f64 {
        self.user_s + self.sys_s
    }
}

/// The `/usr/bin/time` arguments that write the full resource usage to
/// `out`: `-l` on macOS and the BSDs, GNU time's `-v` elsewhere.
#[must_use]
pub fn time_args(out: &std::path::Path) -> Vec<std::ffi::OsString> {
    let flag = if cfg!(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    )) {
        "-l"
    } else {
        "-v"
    };
    vec![flag.into(), "-o".into(), out.as_os_str().to_owned()]
}

/// Parses the output of `/usr/bin/time -l` (BSD and macOS: RSS in bytes) or
/// GNU `/usr/bin/time -v` (RSS in KiB).
#[must_use]
pub fn parse_time_output(text: &str) -> Option<Usage> {
    // BSD: "        1.23 real         0.98 user         0.01 sys"
    //      "    4931584  maximum resident set size"
    let mut user = None;
    let mut sys = None;
    let mut rss = None;
    for line in text.lines() {
        let t = line.trim();
        let words: Vec<&str> = t.split_whitespace().collect();
        if words.len() == 6 && words[1] == "real" && words[3] == "user" && words[5] == "sys" {
            user = words[2].parse().ok();
            sys = words[4].parse().ok();
        } else if let Some(n) = t.strip_suffix("maximum resident set size") {
            rss = n.trim().parse().ok();
        } else if let Some(v) = t.strip_prefix("User time (seconds):") {
            user = v.trim().parse().ok();
        } else if let Some(v) = t.strip_prefix("System time (seconds):") {
            sys = v.trim().parse().ok();
        } else if let Some(v) = t.strip_prefix("Maximum resident set size (kbytes):") {
            rss = v.trim().parse::<u64>().ok().map(|k| k * 1024);
        }
    }
    Some(Usage {
        user_s: user?,
        sys_s: sys?,
        max_rss_bytes: rss?,
    })
}

/// The median, or `None` for no samples.
#[must_use]
pub fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        f64::midpoint(v[n / 2 - 1], v[n / 2])
    })
}

/// The minimum and maximum, or `None` for no samples.
#[must_use]
pub fn range(xs: &[f64]) -> Option<(f64, f64)> {
    let lo = xs.iter().copied().min_by(f64::total_cmp)?;
    let hi = xs.iter().copied().max_by(f64::total_cmp)?;
    Some((lo, hi))
}

/// A tool's single-worker numbers from the survey's `bench-summary.csv`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurveyPoint {
    /// Median wall seconds.
    pub wall_s: f64,
    /// Median peak RSS in MB.
    pub rss_mb: f64,
}

/// Looks up `tool`'s one-worker row for `spec` in the survey's
/// `bench-summary.csv` (columns `spec,tool,workers,reps,reps_correct,
/// wall_median_s,wall_min_s,wall_max_s,peak_rss_median_mb,...`).
#[must_use]
pub fn survey_w1(csv: &str, spec: &str, tool: &str) -> Option<SurveyPoint> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (c_spec, c_tool, c_workers, c_wall, c_rss) = (
        col("spec")?,
        col("tool")?,
        col("workers")?,
        col("wall_median_s")?,
        col("peak_rss_median_mb")?,
    );
    lines.find_map(|l| {
        let f: Vec<&str> = l.split(',').collect();
        (f.get(c_spec) == Some(&spec)
            && f.get(c_tool) == Some(&tool)
            && f.get(c_workers) == Some(&"1"))
        .then(|| {
            Some(SurveyPoint {
                wall_s: f.get(c_wall)?.parse().ok()?,
                rss_mb: f.get(c_rss)?.parse().ok()?,
            })
        })
        .flatten()
    })
}

/// Parses TLC's `N states generated, M distinct states found` line.
#[must_use]
pub fn tlc_counts(stdout: &str) -> Option<(u64, u64)> {
    stdout.lines().rev().find_map(|l| {
        let (gen_part, rest) = l.split_once(" states generated, ")?;
        let generated = gen_part.rsplit(' ').next()?.replace(',', "").parse().ok()?;
        let distinct = rest.split(' ').next()?.replace(',', "").parse().ok()?;
        Some((generated, distinct))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bsd_time() -> Result<(), String> {
        let t = "      106.05 real        41.34 user         0.29 sys\n\
                 4931584  maximum resident set size\n  0  average shared memory size\n";
        let u = parse_time_output(t).ok_or("no usage")?;
        assert!((u.cpu_s() - 41.63).abs() < 1e-9);
        assert_eq!(u.max_rss_bytes, 4_931_584);
        Ok(())
    }

    #[test]
    fn parses_gnu_time() -> Result<(), String> {
        let t = "\tCommand being timed: \"dive\"\n\tUser time (seconds): 1.50\n\
                 \tSystem time (seconds): 0.25\n\tMaximum resident set size (kbytes): 2048\n";
        let u = parse_time_output(t).ok_or("no usage")?;
        assert!((u.cpu_s() - 1.75).abs() < 1e-9);
        assert_eq!(u.max_rss_bytes, 2048 * 1024);
        Ok(())
    }

    #[test]
    fn incomplete_time_output_is_none() {
        assert_eq!(parse_time_output("1.0 real 1.0 user 0.0 sys\n"), None);
    }

    #[test]
    fn statistics() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[4.0, 1.0]), Some(2.5));
        assert_eq!(range(&[4.0, 1.0, 3.0]), Some((1.0, 4.0)));
    }

    #[test]
    fn survey_lookup() {
        let csv = "spec,tool,workers,reps,reps_correct,wall_median_s,wall_min_s,wall_max_s,peak_rss_median_mb\n\
                   MCBakery,tlc,1,3,3,11.68,7.59,12.45,829.0\n\
                   MCBakery,tlc,2,3,3,9.27,6.26,10.5,838.9\n";
        assert_eq!(
            survey_w1(csv, "MCBakery", "tlc"),
            Some(SurveyPoint {
                wall_s: 11.68,
                rss_mb: 829.0
            })
        );
        assert_eq!(survey_w1(csv, "MCBakery", "tla-mc"), None);
    }

    #[test]
    fn tlc_count_line() {
        let out = "@!@!@STARTMSG 2199:0 @!@!@\n131,072 states generated, 65,536 distinct states found, 0 states left on queue.\n";
        assert_eq!(tlc_counts(out), Some((131_072, 65_536)));
    }

    #[test]
    fn rejects_duplicates() {
        let t = "[[bench]]\nname = \"a\"\nmodel = \"m.cfg\"\nwhy = \"\"\n[[bench]]\nname = \"a\"\nmodel = \"m.cfg\"\nwhy = \"\"\n";
        assert!(load_benchmarks(t).is_err());
    }

    #[test]
    fn the_committed_set_parses() -> Result<(), String> {
        let set = load_benchmarks(include_str!("../benchmarks.toml"))?;
        assert!(set.len() >= 6);
        Ok(())
    }
}
