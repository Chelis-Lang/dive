//! Select and verify only the inputs named by the conformance manifests.

use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{Result, run_capture};

pub(super) struct Inputs {
    pub java: PathBuf,
    pub jar: PathBuf,
    pub libraries: Vec<PathBuf>,
}

fn manifest(path: &Path) -> Result<toml::Table> {
    super::read(path)?
        .parse()
        .map_err(|e| format!("invalid pin manifest {}: {e}", path.display()))
}

fn entries<'a>(table: &'a toml::Table, name: &str) -> Result<Vec<&'a toml::Table>> {
    table
        .get(name)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("missing pin list {name}"))?
        .iter()
        .map(|entry| {
            entry
                .as_table()
                .ok_or_else(|| format!("invalid pin entry {name}"))
        })
        .collect()
}

fn string<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("missing pin string {key}"))
}

fn hex<'a>(table: &'a toml::Table, key: &str, length: usize) -> Result<&'a str> {
    let text = string(table, key)?;
    if text.len() != length
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!("invalid {key} pin"));
    }
    Ok(text)
}

fn digest(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("cannot read pinned artifact {}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("cannot read pinned artifact {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let mut hex = String::with_capacity(64);
    for byte in hash.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

fn artifact(cache: &Path, pin: &toml::Table) -> Result<PathBuf> {
    let sha = hex(pin, "sha256", 64)?;
    let url = string(pin, "url")?;
    let name = url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| "pinned artifact URL has no filename".to_owned())?;
    let path = cache.join("sha256").join(sha).join(name);
    if digest(&path)? != sha {
        return Err(format!(
            "SHA-256 mismatch for pinned artifact {}",
            path.display()
        ));
    }
    Ok(path)
}

fn checkout(cache: &Path, id: &str, commit: &str) -> Result<PathBuf> {
    let path = cache.join("git").join(id).join(commit);
    if !path.join(".git/dive-checkout-complete").is_file() {
        return Err(format!(
            "pinned checkout {id} missing; run the harness oracle/corpus fetch"
        ));
    }
    let head = run_capture(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(["rev-parse", "HEAD"]),
    )?;
    if head.trim() != commit {
        return Err(format!("pinned checkout {id} is not at {commit}"));
    }
    let changes = run_capture(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&path)
            .args([
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--ignored",
                "--ignore-submodules=all",
            ]),
    )?;
    if !changes.trim().is_empty() {
        return Err(format!("pinned checkout {id} has local changes"));
    }
    Ok(path)
}

pub(super) fn corpus_dir(root: &Path, cache: &Path, id: &str) -> Result<PathBuf> {
    let corpora = manifest(&root.join("conformance/corpora.toml"))?;
    let pin = entries(&corpora, "corpus")?
        .into_iter()
        .find(|pin| string(pin, "id").is_ok_and(|name| name == id))
        .ok_or_else(|| format!("corpus {id} is not pinned"))?;
    checkout(cache, id, hex(pin, "commit", 40)?)
}

fn pinned_java(cache: &Path, oracles: &toml::Table) -> Result<PathBuf> {
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let jre = oracles
        .get("jre")
        .and_then(|v| v.get("platforms"))
        .and_then(|v| v.get(&platform))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("no pinned JRE for {platform}"))?;
    // Match the harness: verify the archive before using its unpacked tree.
    artifact(cache, jre)?;
    super::find_java(&cache.join("jre").join(hex(jre, "sha256", 64)?))
}

pub(super) fn load(root: &Path, cache: &Path) -> Result<Inputs> {
    let oracles = manifest(&root.join("conformance/oracles.toml"))?;
    let defaults: Vec<_> = entries(&oracles, "tlc")?
        .into_iter()
        .filter(|pin| pin.get("default").and_then(toml::Value::as_bool) == Some(true))
        .collect();
    let [oracle] = defaults.as_slice() else {
        return Err("exactly one default TLC oracle is required".to_owned());
    };
    if string(oracle, "role")? != "gating" {
        return Err("default TLC oracle must be gating".to_owned());
    }
    let jar = artifact(cache, oracle)?;
    let java = pinned_java(cache, &oracles)?;
    let mut libraries = Vec::new();
    for pin in entries(&oracles, "module_library")? {
        let dir = checkout(cache, string(pin, "id")?, hex(pin, "commit", 40)?)?
            .join(string(pin, "subdir")?);
        if !dir.is_dir() {
            return Err(format!("pinned module library {} missing", dir.display()));
        }
        libraries.push(dir);
    }
    libraries.push(corpus_dir(root, cache, "community-modules")?.join("modules"));
    if libraries.iter().any(|path| !path.is_dir()) {
        return Err("pinned CommunityModules library missing".to_owned());
    }
    Ok(Inputs {
        java,
        jar,
        libraries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ABC_SHA: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Result<Self> {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "dive-pins-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Ok(Self(path)),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        fn put(&self, path: &str, bytes: &[u8]) -> Result<PathBuf> {
            let file = self.0.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
            Ok(file)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn corrupt_pinned_artifact_is_refused_without_removing_it() -> Result<()> {
        let cache = Scratch::new()?;
        let pin: toml::Table =
            format!("url = \"https://example.invalid/tool.jar\"\nsha256 = \"{ABC_SHA}\"\n")
                .parse()
                .map_err(|e: toml::de::Error| e.to_string())?;
        let file = cache.put(&format!("sha256/{ABC_SHA}/tool.jar"), b"abc")?;
        assert_eq!(artifact(&cache.0, &pin)?, file);
        cache.put("sha256/other/tool.jar", b"unrelated")?;
        std::fs::write(&file, b"bad").map_err(|e| e.to_string())?;
        assert!(artifact(&cache.0, &pin).is_err());
        assert_eq!(std::fs::read(&file).map_err(|e| e.to_string())?, b"bad");
        Ok(())
    }

    #[test]
    fn unrelated_cached_jre_never_substitutes_for_missing_pin() -> Result<()> {
        let cache = Scratch::new()?;
        let pins: toml::Table = format!(
            "[jre.platforms.{}-{}]\nurl = \"https://example.invalid/jre.tar.gz\"\nsha256 = \"{ABC_SHA}\"\n",
            std::env::consts::OS, std::env::consts::ARCH
        ).parse().map_err(|e: toml::de::Error| e.to_string())?;
        cache.put(&format!("sha256/{ABC_SHA}/jre.tar.gz"), b"abc")?;
        cache.put("jre/other/jdk/bin/java", b"other runtime")?;
        assert!(pinned_java(&cache.0, &pins).is_err());
        let java = cache.put(&format!("jre/{ABC_SHA}/jdk/bin/java"), b"selected runtime")?;
        assert_eq!(pinned_java(&cache.0, &pins)?, java);
        Ok(())
    }

    #[test]
    fn checkout_refuses_wrong_head_and_ignored_local_inputs() -> Result<()> {
        let cache = Scratch::new()?;
        let repo = cache.0.join("fixture");
        run_capture(Command::new("git").args(["init", "-q"]).arg(&repo))?;
        std::fs::write(repo.join("Module.tla"), b"pinned").map_err(|e| e.to_string())?;
        run_capture(
            Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["add", "Module.tla"]),
        )?;
        run_capture(
            Command::new("git")
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-C",
                ])
                .arg(&repo)
                .args(["commit", "-q", "-m", "test fixture"]),
        )?;
        let commit = run_capture(
            Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["rev-parse", "HEAD"]),
        )?;
        let path = cache.0.join("git/input").join(commit.trim());
        std::fs::create_dir_all(cache.0.join("git/input")).map_err(|e| e.to_string())?;
        std::fs::rename(&repo, &path).map_err(|e| e.to_string())?;
        std::fs::write(path.join(".git/dive-checkout-complete"), b"").map_err(|e| e.to_string())?;
        assert_eq!(checkout(&cache.0, "input", commit.trim())?, path);
        std::fs::write(path.join(".git/info/exclude"), b"foreign\n").map_err(|e| e.to_string())?;
        std::fs::write(path.join("foreign"), b"untracked ignored input")
            .map_err(|e| e.to_string())?;
        assert!(
            checkout(&cache.0, "input", commit.trim()).is_err_and(|e| e.contains("local changes"))
        );
        std::fs::remove_file(path.join("foreign")).map_err(|e| e.to_string())?;
        run_capture(
            Command::new("git")
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-C",
                ])
                .arg(&path)
                .args(["commit", "--allow-empty", "-q", "-m", "different revision"]),
        )?;
        assert!(checkout(&cache.0, "input", commit.trim()).is_err_and(|e| e.contains("is not at")));
        Ok(())
    }
}
