# 0144: Explicit PlusCal file publication

- **Status:** accepted
- **Date:** 2026-11-03

## Context

The translator already returns admitted in-memory modules, checksummed rewrites,
original ASTs and effective filesystem preferences. Ordinary translate/write/check
workflows still require callers to implement backups and cfg merging themselves.
Pinned `pcal.trans` at `94d0c507` creates or replaces `.old`, replaces the generated
cfg prefix before its delimiter while preserving the user suffix, and adds a
SPECIFICATION only when the suffix lacks an uncommented occurrence. Its
`-writeAST` operation writes `AST.tla` in the current directory and exits 255,
without translating or creating the module backup/configuration.

## Decision

Keep all existing translation and rewrite operations in memory. Add explicit
`--write`, `prepare_translation_file` and `write_translation_file`. Preparation
uses the existing complete-module admission and modified-body policy, then plans
backup, configuration and module writes with effective source/CLI options.
Publication compares all source/destination bytes again, stages all outputs in
their destination directories, and publishes the module last. Each replacement
is atomic; an ordinary commit failure rolls back earlier replacements. This is
not a cross-file crash-atomic transaction or an editor lock. Failed rollback
retains original staged bytes and reports the recovery path. Concurrent edits
observed before commit refuse without publication. Missing/invalid modules never
write files. Readable regular UTF-8 files are bounded to 4 MiB; symlink/directory
publication targets refuse rather than following an uncertain alias.

A separate explicit `write_original_ast_file` operation and CLI `-writeAST`
write the pre-generation AST. Successful writes retain pinned exit 255 and
`accepted: false`; they do not imply generated-module admission or a model proof.
`--ast` stays an in-memory operation. Reports retain schema 3 with additive actual
write disposition; existing operations and outcomes keep their meanings.

## Consequences

Users can translate and check a file without Java or custom publication code.
Rust callers can inspect a prepared report before publishing and concurrent-edit
failures are directly testable. `.old` retains exact input bytes and permissions;
existing cfg user sections and SPECIFICATION directives survive regeneration.
Default rewriting preserves source newline policy; TLC mode retains the existing
LF rewrite policy. Atomic staging is an implementation safeguard, not a promise
of a multi-file filesystem transaction across a crash. Mapping-file and
specification-backed translator support remain separate work.
