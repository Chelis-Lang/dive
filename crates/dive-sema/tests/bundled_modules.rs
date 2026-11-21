//! Bundled libraries are available without paths and never replace user sources.

use dive_sema::{Options, Origin, SearchPath, analyze_file, analyze_source};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Directory(PathBuf);

impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-bundled-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bundled_modules_resolve_without_search_paths() {
    let analysis = analyze_source(
        "Root.tla",
        "---- MODULE Root ----\nEXTENDS TLAPS, SequencesExt, FiniteSetTheorems, FramedFunctions\n====\n",
        &Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    for name in [
        "TLAPS",
        "SequencesExt",
        "FiniteSetTheorems",
        "FramedFunctions",
    ] {
        let file = analysis
            .files()
            .iter()
            .find(|f| f.display.ends_with(&format!("/{name}.tla")));
        assert!(
            matches!(file, Some(f) if f.origin == Origin::Bundled),
            "{name}"
        );
    }
}

#[test]
fn existing_search_directories_override_bundled_modules() -> TestResult {
    for location in ["root", "library", "module"] {
        let root = Directory::new()?;
        let supplied = Directory::new()?;
        let source = "---- MODULE Functions ----\nMarker == 17\n====\n";
        let override_dir = if location == "root" {
            root.path()
        } else {
            supplied.path()
        };
        std::fs::write(override_dir.join("Functions.tla"), source)?;
        let file = root.path().join("Root.tla");
        std::fs::write(
            &file,
            "---- MODULE Root ----\nEXTENDS Functions\nInv == Marker = 17\n====\n",
        )?;
        let search = SearchPath {
            library_paths: if location == "library" {
                vec![supplied.0.clone()]
            } else {
                vec![]
            },
            after_standard: if location == "module" {
                vec![supplied.0.clone()]
            } else {
                vec![]
            },
        };
        let analysis = analyze_file(
            &file,
            &Options {
                search,
                ..Options::default()
            },
        )?;
        assert!(
            analysis.accepted(),
            "{location}: {:?}",
            analysis.diagnostics()
        );
        assert!(
            analysis
                .files()
                .iter()
                .any(|f| f.text == source && matches!(f.origin, Origin::File(_))),
            "{location}"
        );
    }
    Ok(())
}

#[test]
fn monolith_modules_override_bundled_modules() -> TestResult {
    let root = Directory::new()?;
    let file = root.path().join("Root.tla");
    std::fs::write(
        &file,
        "---- MODULE Root ----\nEXTENDS Functions\nInv == Marker = 17\n====\n---- MODULE Functions ----\nMarker == 17\n====\n",
    )?;
    let analysis = analyze_file(&file, &Options::default())?;
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    assert!(
        analysis
            .files()
            .iter()
            .any(|f| matches!(f.origin, Origin::Monolith(_)) && f.text.contains("Marker == 17"))
    );
    Ok(())
}

#[test]
fn standard_modules_keep_priority_over_their_bundled_copies() {
    let analysis = analyze_source(
        "Root.tla",
        "---- MODULE Root ----\nEXTENDS Bags, FiniteSets, RealTime\n====\n",
        &Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    for name in ["Bags", "FiniteSets", "RealTime"] {
        assert!(
            analysis
                .files()
                .iter()
                .any(|f| f.display == format!("<standard>/{name}.tla")
                    && f.origin == Origin::Standard),
            "{name}"
        );
    }
}
