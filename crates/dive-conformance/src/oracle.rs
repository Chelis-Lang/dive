//! A ready-to-run TLC/SANY oracle: verified artifacts, a Java runtime, the
//! classpath, and the oracle's probed capabilities.
//!
//! # How TLC is invoked
//!
//! This mirrors tlaplus/Examples' `.github/scripts/tla_utils.py`
//! (`check_model`): the working directory is the corpus root, the module and
//! config are passed as corpus-relative paths, the JVM gets
//! `-XX:+UseParallelGC` and the `-cp` jar list, `-Dtlc2.tool.impl.Tool.cdot=true`
//! is added for modules with the `action composition` feature, and TLC gets
//! `-config <cfg> -workers <n> -lncheck final -cleanup` plus `-simulate
//! num=<traceCount>` in simulation mode. Deadlock checking is left at TLC's
//! default (on), exactly as upstream; models that do not want it say so in
//! their `.cfg`.
//!
//! The harness then adds:
//! - `-tool`, for the structured message stream;
//! - `-workers 1` for goldens (ADR 0004);
//! - `-metadir <unique tmp dir>` and `-cleanup`, so nothing is written into
//!   the corpus;
//! - `-fp 0` and `-seed 0`, always: TLC otherwise picks a random fingerprint
//!   polynomial and a random seed per run, and the seed matters in exhaustive
//!   search too (`RandomElement`, `Randomization`; Examples `SpanTreeRandom`);
//! - `-dumpTrace json <file>` where the oracle supports it (not v1.7.4);
//! - `-Xmx`, configurable, and `-Xss` pinned ([`JVM_STACK`]).
//!
//! Every job runs in a fresh scratch copy of the part of the corpus it needs
//! ([`isolate`]); the cached checkout is never a working directory.
//!
//! It deliberately does *not* copy upstream's `-Dtlc2.TLC.ide=Github` and
//! `-Dutil.ExecutionStatisticsCollector.id=...`: the latter makes TLC report
//! execution statistics under upstream CI's identifier. Instead `user.home`
//! points at a harness directory whose `.tlaplus/esc.txt` opts out of
//! collection entirely. `-noGenerateSpecTE` is not passed: v1.7.4 rejects it,
//! v1.7.4 never generates trace-exploration specs unasked, and in `94d0c50`
//! `-tool` already disables generation (`TLC.java`, `generateTESpec`).
//! The JVM's locale, file encoding and temporary directory are pinned
//! (`-Duser.language=en -Duser.country=US -Dfile.encoding=UTF-8
//! -Djava.io.tmpdir=<run dir>`) so number formatting and module decoding do
//! not depend on the host. `JAVA_TOOL_OPTIONS`, `_JAVA_OPTIONS`,
//! `JDK_JAVA_OPTIONS` and `CLASSPATH` are removed from the environment.

use crate::cache::Cache;
use crate::config::{Config, ModuleLibrary, TlcOracle};
use crate::error::{Error, Result};
use crate::fetch::{find_java, verified_checkout};
use crate::hash::sha256_file;
use crate::process::{ProcessOutcome, run_with_timeout};
use crate::util::{TempDir, capture, redact};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Where the Java runtime came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "source")]
pub enum JavaSource {
    /// The pinned Temurin JRE.
    Pinned {
        /// Temurin version.
        version: String,
        /// SHA-256 of the archive it was unpacked from.
        sha256: String,
        /// Platform key.
        platform: String,
    },
    /// A system Java chosen with `--java` (explicit opt-in).
    System {
        /// The path given.
        path: String,
        /// SHA-256 of the executable (a weak identity; recorded for audit).
        sha256: String,
    },
}

/// The Java runtime used to run the oracles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Java {
    /// Path to the `java` executable (not serialised; machine-specific).
    #[serde(skip)]
    pub path: PathBuf,
    /// Where it came from.
    #[serde(flatten)]
    pub source: JavaSource,
    /// First line of `java -version`.
    pub version_line: String,
}

impl Java {
    /// Resolves the pinned JRE from the cache (verifying the archive), or the
    /// explicit `--java` override.
    ///
    /// # Errors
    /// Fails if the JRE is missing or corrupt, or `java -version` fails.
    pub fn resolve(config: &Config, cache: &Cache, override_path: Option<&Path>) -> Result<Self> {
        let (path, source) = if let Some(p) = override_path {
            let sha256 = sha256_file(p)?;
            (
                p.to_path_buf(),
                JavaSource::System {
                    path: p.display().to_string(),
                    sha256,
                },
            )
        } else {
            let jre = config.oracles.host_jre()?;
            cache.verified_artifact("JRE archive", &jre.url, &jre.sha256)?;
            let java = find_java(&cache.jre_dir(&jre.sha256)).map_err(|_| {
                Error::Config(
                    "the pinned JRE is not unpacked; run `dive-conformance oracle fetch`"
                        .to_owned(),
                )
            })?;
            (
                java,
                JavaSource::Pinned {
                    version: config.oracles.jre.version.clone(),
                    sha256: jre.sha256.clone(),
                    platform: crate::config::host_platform_key(),
                },
            )
        };
        let out = Command::new(&path)
            .arg("-version")
            .output()
            .map_err(|e| Error::Command(format!("cannot run {}: {e}", path.display())))?;
        if !out.status.success() {
            return Err(Error::Command(format!(
                "{} -version failed: {}",
                path.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let version_line = String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        Ok(Self {
            path,
            source,
            version_line,
        })
    }
}

/// Features an oracle has, probed by running it (`tlc2.TLC -help` and
/// `tla2sany.SANY -error-codes` on an empty module). Recorded in provenance
/// so that a missing feature shows up as data rather than an obscure
/// failure. `-generate` is not probed: `94d0c50` accepts it but does not
/// list it in its usage text, and generate models are out of scope anyway.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent, separately probed oracle fact"
)]
pub struct Capabilities {
    /// `-dumpTrace json <file>` (absent from v1.7.4).
    pub dump_trace_json: bool,
    /// `-noGenerateSpecTE` (absent from v1.7.4; never passed).
    pub no_generate_spec_te: bool,
    /// SANY's `-error-codes` (absent from v1.7.4, which rejects it).
    pub sany_error_codes: bool,
    /// Whether a temporal-property violation names the property (v1.7.4
    /// prints "Temporal properties were violated." without a name). Probed
    /// by model checking a one-line liveness violation, which also proves
    /// that `-seed` is accepted in breadth-first mode.
    #[serde(default)]
    pub names_temporal_properties: bool,
    /// `X-Git-Revision` from the jar manifest.
    pub git_revision: Option<String>,
    /// `Implementation-Version` from the jar manifest.
    pub implementation_version: Option<String>,
}

/// An identified classpath entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClasspathEntry {
    /// Pin id.
    pub id: String,
    /// SHA-256 for jars, commit for module libraries.
    pub pin: String,
    /// Path (not serialised).
    #[serde(skip)]
    pub path: PathBuf,
}

/// A ready-to-run oracle.
#[derive(Debug, Clone)]
pub struct OracleEnv {
    /// The pin.
    pub oracle: TlcOracle,
    /// The verified jar.
    pub jar: PathBuf,
    /// Classpath jars, in order.
    pub classpath: Vec<ClasspathEntry>,
    /// Module library directories, in order.
    pub libraries: Vec<ClasspathEntry>,
    /// The Java runtime.
    pub java: Java,
    /// Probed capabilities.
    pub capabilities: Capabilities,
    /// Hermetic `user.home`.
    pub home: PathBuf,
    /// Scratch root for runs.
    pub tmp_root: PathBuf,
    /// Cache root (for redaction).
    pub cache_root: PathBuf,
}

/// The mode a model is checked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    /// Exhaustive breadth-first search.
    Exhaustive,
    /// `-simulate num=<traces>`.
    Simulate {
        /// Number of traces.
        traces: u64,
    },
}

/// One TLC invocation.
#[derive(Debug, Clone)]
pub struct TlcJob<'a> {
    /// Corpus checkout root (the working directory).
    pub corpus_root: &'a Path,
    /// Module path relative to the corpus root.
    pub module: &'a str,
    /// Config path relative to the corpus root.
    pub config: &'a str,
    /// Check mode.
    pub mode: CheckMode,
    /// Module features from the manifest.
    pub features: &'a [String],
    /// `-workers`.
    pub workers: u32,
    /// Harness timeout.
    pub timeout: Duration,
    /// `-Xmx` value, e.g. `3g`.
    pub xmx: &'a str,
}

/// The raw outcome of one oracle invocation.
#[derive(Debug, Clone)]
pub struct Invocation {
    /// The command line with machine paths replaced by placeholders.
    pub command: Vec<String>,
    /// How the process ended.
    pub outcome: ProcessOutcome,
    /// The `-dumpTrace json` file, if written.
    pub trace_json: Option<String>,
}

/// The JVM thread stack size, pinned so that deep recursion overflows (or
/// not) the same way on every machine. A stack overflow is a
/// `resource-limit`, never an oracle fact.
pub const JVM_STACK: &str = "8m";

const SCRUBBED_ENV: &[&str] = &[
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "CLASSPATH",
];

impl OracleEnv {
    /// Resolves oracle `id`: verifies its jar, classpath jars, module
    /// libraries and Java, and probes capabilities.
    ///
    /// # Errors
    /// Fails if any artifact is missing or fails verification.
    pub fn prepare(config: &Config, cache: &Cache, id: &str, java: &Java) -> Result<Self> {
        let oracle = config.oracles.oracle(id)?.clone();
        let jar =
            cache.verified_artifact(&format!("TLC oracle `{id}`"), &oracle.url, &oracle.sha256)?;
        let mut classpath = Vec::new();
        for cp in &oracle.classpath {
            let a = config.oracles.classpath_artifact(cp)?;
            let path =
                cache.verified_artifact(&format!("classpath jar `{cp}`"), &a.url, &a.sha256)?;
            classpath.push(ClasspathEntry {
                id: a.id.clone(),
                pin: a.sha256.clone(),
                path,
            });
        }
        let mut libraries = Vec::new();
        for lib in &config.oracles.module_library {
            libraries.push(library_entry(cache, lib)?);
        }
        let mut env = Self {
            oracle,
            jar,
            classpath,
            libraries,
            java: java.clone(),
            capabilities: Capabilities::default(),
            home: cache.tlc_home()?,
            tmp_root: cache.tmp_dir(),
            cache_root: cache.root().to_path_buf(),
        };
        env.capabilities = env.probe()?;
        Ok(env)
    }

    /// The `-cp` value: jar, classpath jars, module libraries.
    #[must_use]
    pub fn classpath_string(&self) -> String {
        let sep = if cfg!(windows) { ";" } else { ":" };
        std::iter::once(self.jar.display().to_string())
            .chain(self.classpath.iter().map(|c| c.path.display().to_string()))
            .chain(self.libraries.iter().map(|c| c.path.display().to_string()))
            .collect::<Vec<_>>()
            .join(sep)
    }

    fn base_command(&self, scratch: &Path, cwd: &Path, xmx: &str) -> Command {
        let mut cmd = Command::new(&self.java.path);
        cmd.current_dir(cwd);
        for var in SCRUBBED_ENV {
            cmd.env_remove(var);
        }
        cmd.arg("-XX:+UseParallelGC")
            .arg(format!("-Xmx{xmx}"))
            .arg(format!("-Xss{JVM_STACK}"))
            .arg(format!("-Duser.home={}", self.home.display()))
            .arg(format!("-Djava.io.tmpdir={}", scratch.display()))
            .arg("-Duser.language=en")
            .arg("-Duser.country=US")
            .arg("-Dfile.encoding=UTF-8");
        cmd
    }

    fn redacted_command(&self, cmd: &Command, scratch: &Path, cwd: &Path) -> Vec<String> {
        let reps = [
            (scratch.to_path_buf(), "$TMP"),
            (self.cache_root.clone(), "$DIVE_CACHE_DIR"),
            (cwd.to_path_buf(), "$CWD"),
        ];
        std::iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(|a| redact(&a.to_string_lossy(), &reps))
            .collect()
    }

    /// Replaces machine paths in captured output with placeholders, so
    /// recorded text (SANY errors, fixtures) is machine-independent.
    fn redact_outcome(&self, outcome: &mut ProcessOutcome, scratch: &Path, cwd: &Path) {
        let reps = [
            (scratch.to_path_buf(), "$TMP"),
            (self.cache_root.clone(), "$DIVE_CACHE_DIR"),
            (cwd.to_path_buf(), "$CWD"),
        ];
        outcome.stdout = redact(&outcome.stdout, &reps);
        outcome.stderr = redact(&outcome.stderr, &reps);
    }

    fn probe(&self) -> Result<Capabilities> {
        let scratch = TempDir::new_in(&self.tmp_root, "probe")?;
        let mut cmd = self.base_command(scratch.path(), scratch.path(), "256m");
        cmd.arg("-cp").arg(&self.jar).arg("tlc2.TLC").arg("-help");
        let help = run_with_timeout(&mut cmd, Duration::from_secs(120), scratch.path())?;
        let usage = format!("{}{}", help.stdout, help.stderr);
        if !usage.contains("-workers") {
            return Err(Error::Command(format!(
                "oracle `{}` did not print its usage for -help: {}",
                self.oracle.id,
                usage.chars().take(400).collect::<String>()
            )));
        }
        let probe_tla = scratch.path().join("DiveProbe.tla");
        std::fs::write(&probe_tla, "---- MODULE DiveProbe ----\n====\n")
            .map_err(|e| Error::io(&probe_tla, &e))?;
        let mut cmd = self.base_command(scratch.path(), scratch.path(), "256m");
        cmd.arg("-cp")
            .arg(&self.jar)
            .arg("tla2sany.SANY")
            .arg("-error-codes")
            .arg("DiveProbe.tla");
        let sany = run_with_timeout(&mut cmd, Duration::from_secs(120), scratch.path())?;
        let probe_dir = scratch.path().join("DiveLive");
        std::fs::create_dir_all(&probe_dir).map_err(|e| Error::io(&probe_dir, &e))?;
        for (name, text) in [
            (
                "DiveLive.tla",
                "---- MODULE DiveLive ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nSpec == Init /\\ [][Next]_x\nDiveLiveProp == <>(x = 1)\n====\n",
            ),
            (
                "DiveLive.cfg",
                "SPECIFICATION Spec\nPROPERTY DiveLiveProp\n",
            ),
        ] {
            let f = probe_dir.join(name);
            std::fs::write(&f, text).map_err(|e| Error::io(&f, &e))?;
        }
        let mut cmd = self.base_command(scratch.path(), scratch.path(), "256m");
        cmd.arg("-cp")
            .arg(&self.jar)
            .arg("tlc2.TLC")
            .args(["DiveLive/DiveLive.tla", "-config", "DiveLive/DiveLive.cfg"])
            .args([
                "-workers", "1", "-tool", "-fp", "0", "-seed", "0", "-cleanup",
            ])
            .arg("-metadir")
            .arg(scratch.path().join("live-states"));
        let live = run_with_timeout(&mut cmd, Duration::from_secs(120), scratch.path())?;
        if live.exit_code != Some(13) {
            return Err(Error::Command(format!(
                "oracle `{}` did not report the probe liveness violation (exit {:?}): {}",
                self.oracle.id,
                live.exit_code,
                live.stdout.chars().take(600).collect::<String>()
            )));
        }
        let manifest = capture(
            Command::new("unzip")
                .arg("-p")
                .arg(&self.jar)
                .arg("META-INF/MANIFEST.MF"),
        )
        .ok();
        let field = |key: &str| {
            manifest.as_deref().and_then(|m| {
                m.lines()
                    .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_owned()))
            })
        };
        Ok(Capabilities {
            dump_trace_json: usage.contains("-dumpTrace"),
            no_generate_spec_te: usage.contains("-noGenerateSpecTE"),
            sany_error_codes: sany.exit_code == Some(0) && !sany.stdout.contains("Illegal switch"),
            names_temporal_properties: live
                .stdout
                .contains("Temporal property DiveLiveProp was violated"),
            git_revision: field("X-Git-Revision:"),
            implementation_version: field("Implementation-Version:"),
        })
    }

    /// Runs TLC on one model.
    ///
    /// # Errors
    /// Fails only if the process cannot be started or scratch space cannot
    /// be created; every TLC outcome, including timeouts, is returned.
    pub fn run_tlc(&self, job: &TlcJob<'_>) -> Result<Invocation> {
        let scratch = TempDir::new_in(&self.tmp_root, "tlc")?;
        let work = isolate(job.corpus_root, job.module, scratch.path())?;
        let mut cmd = self.base_command(scratch.path(), &work, job.xmx);
        if job.features.iter().any(|f| f == "action composition") {
            cmd.arg("-Dtlc2.tool.impl.Tool.cdot=true");
        }
        cmd.arg("-cp").arg(self.classpath_string()).arg("tlc2.TLC");
        cmd.arg(job.module).args(["-config", job.config]);
        // `-simulate num=N` must not be the last argument: v1.7.4 consumes
        // `num=N` only when another argument follows it (`TLC.java`,
        // `(index + 1) < args.length`), and otherwise rejects the command
        // line (WRONG_COMMANDLINE_PARAMS_TLC, exit 1).
        if let CheckMode::Simulate { traces } = job.mode {
            cmd.args(["-simulate", &format!("num={traces}")]);
        }
        cmd.args(["-workers", &job.workers.to_string()])
            .args([
                "-lncheck", "final", "-cleanup", "-tool", "-fp", "0", "-seed", "0",
            ])
            .arg("-metadir")
            .arg(scratch.path().join("states"));
        let trace_file = scratch.path().join("trace.json");
        if self.capabilities.dump_trace_json {
            cmd.args(["-dumpTrace", "json"]).arg(&trace_file);
        }
        let command = self.redacted_command(&cmd, scratch.path(), &work);
        let mut outcome = run_with_timeout(&mut cmd, job.timeout, scratch.path())?;
        let trace_json = std::fs::read_to_string(&trace_file).ok();
        self.redact_outcome(&mut outcome, scratch.path(), &work);
        Ok(Invocation {
            command,
            outcome,
            trace_json,
        })
    }

    /// Runs SANY on `module` (corpus-relative) from the module's own
    /// directory inside an isolated copy, with `-error-codes` when the oracle
    /// supports it.
    ///
    /// # Errors
    /// Fails only if the process cannot be started or the copy fails.
    pub fn run_sany(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<Invocation> {
        let scratch = TempDir::new_in(&self.tmp_root, "sany")?;
        let work = isolate(corpus_root, module, scratch.path())?;
        let module_path = work.join(module);
        let module_dir = module_path
            .parent()
            .ok_or_else(|| Error::Config(format!("module `{module}` has no directory")))?
            .to_path_buf();
        let file = module_path
            .file_name()
            .ok_or_else(|| Error::Config(format!("module `{module}` has no file name")))?
            .to_os_string();
        let mut cmd = self.base_command(scratch.path(), &module_dir, "1g");
        cmd.arg("-cp")
            .arg(self.classpath_string())
            .arg("tla2sany.SANY");
        if self.capabilities.sany_error_codes {
            cmd.arg("-error-codes");
        }
        cmd.arg(&file);
        let command = self.redacted_command(&cmd, scratch.path(), &module_dir);
        let mut outcome = run_with_timeout(&mut cmd, timeout, scratch.path())?;
        self.redact_outcome(&mut outcome, scratch.path(), &module_dir);
        Ok(Invocation {
            command,
            outcome,
            trace_json: None,
        })
    }

    /// The machine-independent description of this oracle for provenance.
    #[must_use]
    pub fn provenance(&self) -> OracleProvenance {
        OracleProvenance {
            id: self.oracle.id.clone(),
            sha256: self.oracle.sha256.clone(),
            upstream_commit: self.oracle.upstream_commit.clone(),
            classpath: self.classpath.clone(),
            module_libraries: self.libraries.clone(),
            capabilities: self.capabilities.clone(),
            java: self.java.clone(),
        }
    }
}

/// Copies the part of the corpus a job needs into `<scratch>/work`,
/// preserving its corpus-relative path, and returns `<scratch>/work`. Oracle
/// runs execute there, so nothing they write (TLC metadata, `IOUtils`
/// output, animation SVGs, trace-exploration specs) can reach the cached
/// corpus checkout. The part copied is [`crate::corpus::isolation`] of
/// `path`: the whole spec directory for tlaplus/Examples, else the files of
/// the module's own directory.
///
/// # Errors
/// Fails on any file-system error.
pub fn isolate(corpus_root: &Path, path: &str, scratch: &Path) -> Result<PathBuf> {
    let work = scratch.join("work");
    let (rel, recursive) = crate::corpus::isolation(path);
    let dest = work.join(&rel);
    if recursive {
        crate::util::copy_dir(&corpus_root.join(&rel), &dest)?;
    } else {
        crate::util::copy_dir_files(&corpus_root.join(&rel), &dest)?;
    }
    Ok(work)
}

/// The verified checkout directory of a module library (`[[module_library]]`).
///
/// # Errors
/// Fails if the library is not fetched at its pinned commit.
pub fn library_entry(cache: &Cache, lib: &ModuleLibrary) -> Result<ClasspathEntry> {
    let dir = cache.checkout_dir(&lib.id, &lib.commit);
    verified_checkout(&dir, &lib.commit, &format!("module library `{}`", lib.id))?;
    Ok(ClasspathEntry {
        id: lib.id.clone(),
        pin: lib.commit.clone(),
        path: dir.join(&lib.subdir),
    })
}

/// What a golden or report records about the oracle that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleProvenance {
    /// Oracle id.
    pub id: String,
    /// Jar SHA-256.
    pub sha256: String,
    /// Upstream commit or tag.
    pub upstream_commit: String,
    /// Classpath jars.
    pub classpath: Vec<ClasspathEntry>,
    /// Module libraries.
    pub module_libraries: Vec<ClasspathEntry>,
    /// Probed capabilities, including the jar's `X-Git-Revision`.
    pub capabilities: Capabilities,
    /// The Java runtime.
    pub java: Java,
}
