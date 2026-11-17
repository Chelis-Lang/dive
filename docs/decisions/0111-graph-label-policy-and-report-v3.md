# 0111: Explicit graph-label policy and report major 3

- **Status:** accepted
- **Date:** 2026-10-21

## Context

Packed truth columns store only an atom's actual state or action domain. The
legacy `analyze::Limits::atom_cells` nevertheless admits graph labels using
`(states + retained edges) * atoms`. The same scalar separately bounds
attribution rows and formula-node truth on a periodic word. These are three
owners with different quantities; changing the shared scalar would change all
three and silently ignore custom graph ceilings.

A single instrumented experimental MCCRDT run on source `95f7dc7f` bypassed only
the graph caller's logical ceiling. It completed with the pinned success and
25,000 distinct/1,350,001 generated/depth 16, within explicit native 165,800 ms
and 8 GiB budgets. The preceding unchanged-ceiling diagnostic observed 702,412
retained edges and 20 atoms: 14,548,240 logical cells exceed 8,000,000. These are
new-source diagnostic observations, not production qualification, throughput or
true peak memory measurements. The earlier censored experiment in #389 still
has unknown phase and cause. No experimental bypass is delivery code.

Schema 2 reports the shared `atom_cells` setting. Keeping 8,000,000 there while
disabling its graph owner would misrepresent an effective guard. ADR 0108 freezes
field meanings within a major version. Advance notice, including the exact
field map, was given to the known downstream consumer before implementation.
A consumer's schema-2 pin stays unchanged until its reader migrates.

## Decision

Add `GraphLabelPolicy::Cells(n)` and `NoCellLimit`, selected explicitly through
new final and periodic `*_with_policies` APIs. Every existing Rust entrypoint,
including acceptance-policy entrypoints, selects `Cells(limits.atom_cells)`.
Zero, exact boundaries and explicitly supplied default values retain their
original finite meanings; no sentinel or default-value inference selects a new
policy. `Limits` and the acceptance-policy contracts remain intact.

`NoCellLimit` removes only graph-label aggregate-count admission. It retains the
checked true-edge total, shape checks, packed actual-domain layout arithmetic,
fallible reservations, original evaluation order and run control. Attribution
rows and word truth retain their existing 8,000,000 defaults and custom scalar
values. Capture, tableau, product and acceptance bounds are unchanged.
File final and periodic requests explicitly select `NoCellLimit` in both
profiles. Delivery is conditional on the full gate, both baselines, bounded
qualification of all known affected models in both actual profiles and the
normal-input instruction-overhead acceptance bar. A later refusal or stop earns
no agreement and holds default count removal pending its reviewed resolution.

All public operation writers move together to report major 3. Native trace v1,
conformance reports, goldens and checkpoint formats are independent and unchanged.
Replace `options.temporal_bounds.atom_cells` with `cell_limits` owners:

- `graph_labels`: `{ "kind": "cells", "cells": n }` or
  `{ "kind": "run-budgets" }` for no logical cell ceiling;
- `attribution_rows`: `{ "kind": "cells", "cells": n }`;
- `word_truth`: `{ "kind": "cells", "cells": n }`.

The latter two values are the actual supplied legacy scalar. Reports store the
actual selected configuration, including failures before capture; settings do
not certify execution. `acceptance_policy` distinguishes actual legacy cells
from byte/work buffers. Existing acceptance byte/work fields contain their
actual values in buffer mode and null in legacy mode. Other bound fields retain
their quantities. Strict major-3 readers reject unknown or contradictory
policies and the removed ambiguous field. Historical supported version-1
frontend/safety and version-2 reports remain readable without rewriting them;
study-final retains its version-2 final-mode contract and adds explicit version 3.

No new DFID or simulation outcome is added. Their existing closed dispositions
remain unchanged; #350 needs its own reviewed protocol decision before adding a
stopped disposition. Major 3 is not a reservation of unspecified future outcomes.

## Consequences

The graph owner can grow with its packed representation and existing native
budgets while callers retain exact legacy limits. Consumers migrate before
moving their pin; a producer-only version bump is insufficient. Both check
families remain explicit, and no word becomes a violation before independent
validation completes.

This is not a process-memory projection or a universal interruption guarantee.
Allocation and individual zero-fill/native/quiet-value units remain subject to
#349's documented cooperative scope. Representation payload is not proven RSS
plus swap; no projected bytes are reported as observed memory. Allocation/layout
failure remains its actual named failure, never an invented budget stop. No new
per-state probe, extra exploration, dependency edge or unsafe code is introduced.
