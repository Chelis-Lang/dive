# Core status

Core is the product engine under [ADR 0153](../../decisions/0153-core-is-the-product.md).
The [scoreboard](../../scoreboard.md) holds the current measured results;
[RELEASE.md](../../RELEASE.md) defines the release gates. Issue #498 tracks
those gates, and #784 receives the R1–R3 status and refusal-reason histogram.

## Current gaps

- The product still defaults to Reference and Auto may fall back to it. Core
  must be measured alone against the pinned goldens (#785) before its coverage
  can justify removing that runtime (#791).
- The default-profile canonical-set change is not implemented yet (#786).
  MCQuicksort remains a named refusal; the normalization-history fix stream
  ends under ADR 0153.
- Core liveness cost and capture capacity need measured improvements (#787,
  #788). Work is ranked using actual corpus refusals and stops.
- Bare invocation still needs bundled TLAPS and CommunityModules (#789).
- Nominal compat needs a documented gap list (#790). CLI/report stability and
  release packaging remain open (#792–#794).

## Next bounded actions

Follow the [critical path](../../RELEASE.md#critical-path): measure Core alone,
make default sets canonical, and burn down the largest refusal buckets. Bundle
libraries in parallel. Remove the fallback after R1, then freeze the surface
and ship. Use focused checks and required CI for incremental changes; run full
qualification only for an engine-cutover or release candidate.

Earlier capability and performance narratives remain in git and the existing
reports. They are not additional release gates or work queues.
