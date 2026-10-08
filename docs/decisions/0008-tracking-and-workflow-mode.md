# 0008: Tracking and workflow mode

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Several agents will develop the project over a long period, so tracking must outlive any one session. The owner chose GitHub milestones and issues in this repo over the org's central OpenSpec store. The owner also asked for minimal ceremony while the repo has no users.

## Decision

- **Tracking:**
  - GitHub milestones M0–M5 mirror `docs/ROADMAP.md`;
  - issues are opened per deliverable and per defect, and every conformance disagreement becomes an issue with its minimal reproducing spec;
  - owner questions go in `docs/open-questions.md`.
- **Workflow:**
  - Start in direct-to-main mode. CI still runs the authorship guard over every pushed commit (`before..sha`).
  - Switch to PR mode when parallel agent work needs coordination, expected around M2/M3.
  - The switch is one command, `cargo xtask protect-main`. It requires the `No AI authorship markers` and `Workspace gate` checks, enforced for admins, plus squash merges.
  - `AGENTS.md` records the current mode.

## Consequences

Early velocity stays high. Branch protection is deferred, not dropped. Until the switch, `main` relies on discipline: run the local gate before pushing.

## Update 2026-10-08

PR mode began on the same day. Three parallel agents were running, and the authorship-guard negative test plus an accidental commit had already turned `main` red in two runs. That was enough reason to switch early. Branch protection is now on.
