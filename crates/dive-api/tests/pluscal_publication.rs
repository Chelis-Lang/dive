//! Explicit publication preserves admitted bytes and ordinary translator file effects.
#![allow(clippy::unwrap_used, reason = "test fixture filesystem operations")]
use dive_api::{
    CheckRequest, RewriteRequest, TranslationRequest, prepare_translation_file,
    write_original_ast_file, write_translation_file,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dive-pcal-publication-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("Files.tla")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn source() -> &'static str {
    "---- MODULE Files ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable x=0; begin L: x:=1; E:skip; end algorithm; *)\n====\n"
}
fn no_staging(parent: &Path) {
    assert!(fs::read_dir(parent).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".dive-pcal-")
    }));
}

#[test]
fn prepared_translation_writes_only_on_publish_and_checks_as_a_real_model() {
    for compat in [false, true] {
        let directory = Directory::new();
        let file = directory.file();
        fs::write(&file, source()).unwrap();
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat;
        let prepared = prepare_translation_file(&file, &request).unwrap();
        assert!(
            prepared.report().accepted(),
            "{}",
            prepared.report().to_json()
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), source());
        assert!(!file.with_extension("old").exists());
        assert!(!file.with_extension("cfg").exists());
        let expected = prepared.report().to_json()["replacement"]["module_source"]
            .as_str()
            .unwrap()
            .to_owned();
        let report = prepared.publish().unwrap();
        assert!(report.accepted());
        assert_eq!(
            report.to_json()["filesystem_plan"]["writes_performed"],
            true
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), expected);
        assert_eq!(
            fs::read_to_string(file.with_extension("old")).unwrap(),
            source()
        );
        assert_eq!(
            fs::read_to_string(file.with_extension("cfg")).unwrap(),
            "SPECIFICATION Spec\n\\* Add statements after this line.\n"
        );
        let checked = dive_api::check_file_engine(
            &CheckRequest {
                file,
                compat_tlc: compat,
                seed: Some(7),
                ..CheckRequest::default()
            },
            dive_api::Engine::Reference,
        )
        .unwrap();
        assert_eq!(checked.exit_code(), 0, "{}", checked.to_json());
        assert!(checked.result().is_some());
        no_staging(&directory.0);
    }
}

#[test]
fn cfg_generated_prefix_is_replaced_and_user_suffix_and_existing_spec_survive() {
    for (previous, suffix, has_spec) in [
        (
            "INVARIANT Inv\n\n\\* user comment\n",
            "\\* Add statements after this line.\nINVARIANT Inv\n\n\\* user comment\n",
            false,
        ),
        (
            "PROPERTY Old\n\\* Add statements after this line.\nINVARIANT Inv\n",
            "\\* Add statements after this line.\nINVARIANT Inv\n",
            false,
        ),
        (
            "SPECIFICATION Other\n",
            "\\* Add statements after this line.\nSPECIFICATION Other\n",
            true,
        ),
        (
            "\\* SPECIFICATION Commented\n",
            "\\* Add statements after this line.\n\\* SPECIFICATION Commented\n",
            false,
        ),
    ] {
        let directory = Directory::new();
        let file = directory.file();
        fs::write(&file, source().replace("x=0", "x")).unwrap();
        fs::write(file.with_extension("cfg"), previous).unwrap();
        fs::write(file.with_extension("old"), "obsolete backup").unwrap();
        let mut request = RewriteRequest::default();
        request.translation.limits.check_termination = true;
        let report = write_translation_file(&file, &request).unwrap();
        assert!(report.accepted(), "{}", report.to_json());
        let prefix = if has_spec { "" } else { "SPECIFICATION Spec\n" };
        assert_eq!(
            fs::read_to_string(file.with_extension("cfg")).unwrap(),
            format!(
                "{prefix}PROPERTY Termination\nCONSTANT defaultInitValue = defaultInitValue\n{suffix}"
            )
        );
        assert_eq!(
            fs::read_to_string(file.with_extension("old")).unwrap(),
            source().replace("x=0", "x")
        );
        no_staging(&directory.0);
    }
}

#[test]
fn effective_source_flags_and_default_newlines_drive_actual_files() {
    let directory = Directory::new();
    let file = directory.file();
    let text = source()
        .replace(
            "(* --algorithm",
            "\\* PlusCal options (-noold, -nocfg)\n(* --algorithm",
        )
        .replace('\n', "\r\n");
    fs::write(&file, &text).unwrap();
    fs::write(file.with_extension("cfg"), "existing cfg").unwrap();
    fs::write(file.with_extension("old"), "existing old").unwrap();
    let report = write_translation_file(&file, &RewriteRequest::default()).unwrap();
    assert!(report.accepted(), "{}", report.to_json());
    assert_eq!(
        fs::read_to_string(file.with_extension("cfg")).unwrap(),
        "existing cfg"
    );
    assert_eq!(
        fs::read_to_string(file.with_extension("old")).unwrap(),
        "existing old"
    );
    let output = fs::read_to_string(&file).unwrap();
    assert!(output.contains("\r\n"));
    assert!(!output.replace("\r\n", "").contains('\n'));
    no_staging(&directory.0);
}

#[test]
fn changed_source_cfg_or_backup_refuses_without_overwriting_any_file() {
    for changed in ["tla", "cfg", "old"] {
        let directory = Directory::new();
        let file = directory.file();
        fs::write(&file, source()).unwrap();
        let pending = prepare_translation_file(&file, &RewriteRequest::default()).unwrap();
        assert!(pending.report().accepted());
        fs::write(file.with_extension(changed), "concurrent edit").unwrap();
        let error = pending.publish().unwrap_err();
        assert!(error.to_string().contains("DIVE-P0100"));
        for extension in ["tla", "cfg", "old"] {
            let target = file.with_extension(extension);
            if extension == changed {
                assert_eq!(fs::read_to_string(target).unwrap(), "concurrent edit");
            } else if extension == "tla" {
                assert_eq!(fs::read_to_string(target).unwrap(), source());
            } else {
                assert!(!target.exists());
            }
        }
        no_staging(&directory.0);
    }
}

#[test]
fn failed_admission_and_invalid_destination_make_no_publication() {
    let directory = Directory::new();
    let file = directory.file();
    let invalid = source().replace("x:=1", "x:=UNKNOWN");
    fs::write(&file, &invalid).unwrap();
    let report = write_translation_file(&file, &RewriteRequest::default()).unwrap();
    assert!(!report.accepted());
    assert_eq!(fs::read_to_string(&file).unwrap(), invalid);
    assert!(!file.with_extension("old").exists());
    assert!(!file.with_extension("cfg").exists());
    fs::write(&file, source()).unwrap();
    fs::create_dir(file.with_extension("cfg")).unwrap();
    assert!(prepare_translation_file(&file, &RewriteRequest::default()).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), source());
    assert!(!file.with_extension("old").exists());
    no_staging(&directory.0);
}

#[test]
fn modified_body_policy_is_preserved_by_publication() {
    for compat in [false, true] {
        let directory = Directory::new();
        let file = directory.file();
        fs::write(&file, source()).unwrap();
        assert!(
            write_translation_file(&file, &RewriteRequest::default())
                .unwrap()
                .accepted()
        );
        let edited = fs::read_to_string(&file)
            .unwrap()
            .replace("x' = 1", "x' = 2");
        fs::write(&file, &edited).unwrap();
        let mut request = RewriteRequest::default();
        request.translation.compat_tlc = compat;
        let report = write_translation_file(&file, &request).unwrap();
        assert_eq!(report.accepted(), compat, "{}", report.to_json());
        if compat {
            assert!(fs::read_to_string(&file).unwrap().contains("x' = 1"));
            assert_eq!(
                fs::read_to_string(file.with_extension("old")).unwrap(),
                edited
            );
        } else {
            assert_eq!(fs::read_to_string(&file).unwrap(), edited);
            request.editing.replace_modified = true;
            assert!(write_translation_file(&file, &request).unwrap().accepted());
        }
        no_staging(&directory.0);
    }
}

#[test]
fn ast_write_keeps_pinned_255_and_does_not_translate_or_touch_cfg_backup() {
    let directory = Directory::new();
    let file = directory.file();
    fs::write(&file, source()).unwrap();
    fs::write(file.with_extension("cfg"), "cfg sentinel").unwrap();
    fs::write(file.with_extension("old"), "old sentinel").unwrap();
    let ast = directory.0.join("AST.tla");
    let request = TranslationRequest::default();
    let expected = dive_api::original_ast_file(&file, &request)
        .unwrap()
        .to_json()["ast_artifact"]["module_source"]
        .as_str()
        .unwrap()
        .to_owned();
    let report = write_original_ast_file(&file, &ast, &request).unwrap();
    assert!(report.written());
    assert_eq!(report.exit_code(), 255);
    assert_eq!(report.to_json()["accepted"], false);
    assert_eq!(report.to_json()["generated_module_accepted"], false);
    assert_eq!(fs::read_to_string(ast).unwrap(), expected);
    assert_eq!(fs::read_to_string(&file).unwrap(), source());
    assert_eq!(
        fs::read_to_string(file.with_extension("cfg")).unwrap(),
        "cfg sentinel"
    );
    assert_eq!(
        fs::read_to_string(file.with_extension("old")).unwrap(),
        "old sentinel"
    );
    no_staging(&directory.0);
}

#[test]
fn ast_source_failure_and_source_alias_never_overwrite_existing_files() {
    let directory = Directory::new();
    let file = directory.file();
    let ast = directory.0.join("AST.tla");
    fs::write(
        &file,
        "---- MODULE Files ----\n(* --algorithm A begin L: if end algorithm *)\n====\n",
    )
    .unwrap();
    fs::write(&ast, "AST sentinel").unwrap();
    let report = write_original_ast_file(&file, &ast, &TranslationRequest::default()).unwrap();
    assert!(!report.written());
    assert_eq!(report.exit_code(), 255);
    assert_eq!(fs::read_to_string(&ast).unwrap(), "AST sentinel");
    fs::write(&file, source()).unwrap();
    assert!(
        write_original_ast_file(
            &file,
            &directory.0.join(".").join("Files.tla"),
            &TranslationRequest::default()
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), source());
    no_staging(&directory.0);
}

#[cfg(unix)]
#[test]
fn backups_preserve_private_source_permissions_and_symlinks_refuse() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let directory = Directory::new();
    let file = directory.file();
    fs::write(&file, source()).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        write_translation_file(&file, &RewriteRequest::default())
            .unwrap()
            .accepted()
    );
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(file.with_extension("old"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let alias = directory.0.join("alias.tla");
    symlink(&file, &alias).unwrap();
    assert!(prepare_translation_file(&alias, &RewriteRequest::default()).is_err());
    fs::remove_file(file.with_extension("cfg")).unwrap();
    symlink(&file, file.with_extension("cfg")).unwrap();
    assert!(prepare_translation_file(&file, &RewriteRequest::default()).is_err());
    no_staging(&directory.0);
}
