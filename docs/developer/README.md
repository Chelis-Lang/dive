# Developer documentation map

| Concern | Maintained source |
| --- | --- |
| Contribution, gates, authorship, soundness and layering | [AGENTS.md](../../AGENTS.md) |
| Scope and contract | [Charter](../CHARTER.md) |
| Delivery order and remaining work | [Roadmap](../ROADMAP.md), [parity plan](../design/tlc-parity-plan.md) |
| Current shipped coverage and evidence limits | [Sprint status](../sprint-status.md), [feature ledger](../tlc-parity.md) |
| User contract and runnable usage | [User manual](../user/README.md) |
| Feature design and rationale | [ADRs](../decisions/), [design plans](../design/) |
| TLC limitations and behavior changes | [User-facing differences](../tlc-differences.md), [registry](../divergences.md) |
| Reference source/license provenance | [Provenance](../provenance.md) |
| Reproduction, pins and reports | [Corpus guide](../conformance-corpora.md), [harness README](../../crates/dive-conformance/README.md) |
| Bugs and unanswered scope | GitHub issues, [owner questions](../open-questions.md) |
| CI scheduling and complete test partitions | [CI scheduling](ci.md) |
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
