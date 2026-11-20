# Developer documentation map

| Concern | Maintained source |
| --- | --- |
| Contribution, gates, authorship, soundness and layering | [AGENTS.md](../../AGENTS.md) |
| Scope and contract | [Charter](../CHARTER.md) |
| Delivery order and remaining work | [Roadmap](../ROADMAP.md) |
| Current coverage | [Scoreboard](../scoreboard.md), [conformance summary](../conformance.md) |
| User contract and runnable usage | [User manual](../user/README.md) |
| Feature design and rationale | [ADRs](../decisions/) |
| PlusCal translator | [Translation guide](../pluscal-translation.md), [parser](../pluscal-parser.md) |
| TLC limitations and behavior changes | [User-facing differences](../tlc-differences.md), [registry](../divergences.md) |
| Reference source/license provenance | [Provenance](../provenance.md) |
| Reproduction, pins and reports | [Corpus guide](../conformance-corpora.md), [harness README](../../crates/dive-conformance/README.md) |
| Bugs and unanswered scope | GitHub issues, [owner questions](../open-questions.md) |
| CI jobs and the local full gate | [CI](ci.md) |
| Release procedure and crates.io publication | [Releasing](releasing.md) |
| Benchmark method and measurements | [Performance](performance.md) |
| Release performance runbook, dive and Sail | [Release performance qualification](release-performance-plan.md) |
| Release performance measurements for 0.1.0 | [Release performance results](release-performance-results.md) |
| New execution core: design, status and handoff (ADR 0117) | [Core](core/README.md) |
| Simulation: temporal properties, workers and scale (ADR 0162) | [Simulation design](simulation-design.md) |
| Documentation acceptance | [Documentation policy](documentation-policy.md) |

Parity means observable results (ADR 0086). Committed evidence is compact JSON or
Markdown of at most 100 KB per item; bulky evidence is regenerated from pinned
inputs. Never rewrite generated goldens, omitted cases or failures to promote
coverage. ADR acceptance does not imply implementation completion.

IR traversal can use `Expr::for_each_child` to visit direct expression children
without building a temporary list. `Expr::try_for_each_child` stops at the first
callback error. Both preserve the source order and definition-reference exclusions
of `Expr::children`; the owned-list API remains available. Structural admission
and frame/effects analysis use these visitors without omitting validation checks.
