# Developer documentation map

| Concern | Maintained source |
| --- | --- |
| Contribution, gates, authorship, soundness and layering | [AGENTS.md](../../AGENTS.md) |
| Scope and contract | [Charter](../CHARTER.md) |
| Delivery order and remaining work | [Roadmap](../ROADMAP.md), [parity plan](../design/tlc-parity-plan.md) |
| Current shipped coverage and evidence limits | [Sprint status](../sprint-status.md), [feature ledger](../tlc-parity.md) |
| User contract and runnable usage | [User manual](../user/README.md) |
| Feature design and rationale | [ADRs](../decisions/), [design plans](../design/) |
| Strict tuple-only replay admission | [ADR 0064](../decisions/0064-tuple-only-independent-replay.md), [user boundary](../tuple-only-trace-replay.md) |
| Tuple-variable whole-word admission | [ADR 0067](../decisions/0067-inductive-tuple-replay.md), [fixed projection ADR 0071](../decisions/0071-fixed-tuple-projection-replay.md), [implemented boundary](../inductive-tuple-trace-replay.md), [original producer evidence](../../conformance/probes/inductive-tuple-replay/README.md) |
| TLC limitations and behavior changes | [User-facing differences](../tlc-differences.md), [registry](../divergences.md) |
| Reference source/license provenance | [Provenance](../provenance.md) |
| Reproduction, pins and reports | [Corpus guide](../conformance-corpora.md), [harness README](../../crates/dive-conformance/README.md) |
| Bugs and unanswered scope | GitHub issues, [owner questions](../open-questions.md) |
| CI jobs and the local full gate | [CI](ci.md) |
| Documentation acceptance | [Documentation policy](documentation-policy.md) |

Keep user contracts separate from experiment history. Historical probe READMEs,
source matrices and failed captures describe their original source trees. A new
implementation needs separately identified current-source evidence and truthful
status. The [integration study](../native-parity-integration.md) records that boundary.
Never rewrite generated goldens, omitted cases, failures or original source hashes
to promote coverage. ADR acceptance does not imply implementation completion.

[Ordered temporal execution](../temporal-execution-api.md) documents the staged
source callback and graph contract, independently of the exposed final/short mode.

- [Configured periodic snapshot](../configured-periodic-snapshot.md): original configured/selected body identities, callback-free prebinding policy and ADR 0057; ordinary schedule authority remains unfinished.

[Producer request/claim quiescence](../producer-progress-requests.md), [ADR 0059](../decisions/0059-producer-progress-requests.md) and [complete actual method controls](../../conformance/probes/progress-queue/README.md) track the original #201 boundary. The [committed-prefix guide](../committed-periodic-prefix.md),
[request-service guide](../request-owned-periodic-service.md) and
[candidate-admission guide](../producer-owned-temporal-candidate.md) characterize
the subsequent implemented diagnostic stages. Ordinary scheduling, stopping
counts and temporal completion remain separate unfinished obligations.

The [composed core receiving delivery](../core-prefix-storage-composition.md) now has a separate
1,346-test zero-skip gate and fresh selected release runtimes, with all 37
required models agreeing at 1/4/8 workers. Component and failed producer
identities remain separate; ordinary scheduling and full-family certification
are still open.
