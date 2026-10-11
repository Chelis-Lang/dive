# Names of expanded temporal properties

TLC preserves three separate identities when it binds a configured temporal
property. Explicit bounded final TLC mode now retains all three for the
supported scope. Witness claims still use the original configured properties;
expanded action names and derived role labels describe the binding and planning
order, and never replace those claims.

For example:

```tla
Combine(p, q) == p /\ q
P == Combine([]<>(~x), <>[]x)
```

With `PROPERTY P`, the configured list contains one `P`. TLC expands it into
two temporal Actions, each retaining the original name `P`. Its derived labels
are the two source locations inside `Combine`. Likewise, an alias configured as
`PROPERTY Alias` can keep `Alias` on the Action while its derived local label is
`Saved`. Formula equality cannot establish either name: the binder records the
original name while it recursively processes the property.

## CLI and schema 2

From the repository root:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/tlc-final-property-origins/models/TfBooleanFails/TfBooleanFails.tla \
  --compat=tlc --lncheck=final --lasso=tlc --seed=0 --fp=0 \
  --workers=4 --output=json
```

A completed temporal result adds three arrays under `execution`:

| Field | Meaning in this example |
| --- | --- |
| `configured_property_names` | `["P"]`, before splitting |
| `expanded_property_names` | `["P", "P"]`, original names of expanded temporal Actions |
| `role_names` | Two derived source-location labels in planning order |

The violated trace still claims `P` once. There is no new flag, profile, seed,
resource default or scheduling behavior. Ordinary schema 1 is unchanged. The
safety-only route does not invent temporal Action metadata or a temporal
product; unsupported admissions do not receive completed identity fields.

## Rust callers

A freshly bound model provides
`model.implied_temporal_action_names() -> &[String]`. `phase_l::Session` and successful `phase_execution::Complete` also expose
the ordered expanded names. `tlc_plan::BoundProjection` keeps its existing
public construction contract: inspect the source-bound Model getter alongside
its configured-property and derived-role fields. The final
facade exposes them through the private outcome's actual execution evidence.
Configured properties and derived labels remain separate existing fields.

The names are a binder snapshot, not caller-supplied proof authority. Rebind
rather than editing public role vectors and reusing their old origins. A role
quantity that no longer matches the private origins produces a typed context
refusal before origin-bearing Session execution. Legacy diagnostic
`BoundProjection` still projects formula/label fields without origin metadata. Final checking always performs fresh binding and
independent in-memory and JSON witness replay. Editing a JSON metadata array
cannot create a validated model or change which property the source defines.

## Implemented scope and remaining limits

The paired primary controls cover aliases, local LET definitions, genuine
formal expression forwarding, conjunction splitting, nested INSTANCE bodies
and substitutions. The two exported temporal-formal INSTANCE controls remain
named refusals: the pin itself fails before completing those projections.
No identity agreement is claimed for their missing fields.

This is a final-only metadata contract. Ordinary periodic liveness scheduling,
unsupported higher-order or recursive contexts, I/O, exact temporal storage,
and existing arena/work/graph limits remain unchanged. See the
[final API guide](tlc-final-api.md) and [scoped final guide](tlc-final-scopes.md).
The primary source and raw three-vector projections are retained in the
[origin probe selection](../conformance/probes/tlc-final-property-origins/).

The executable Rust example below uses fresh source binding and final checking
(the complete source is `conformance/probes/tlc-final-property-origins/user-example.rs`).
Run it in a Cargo package using the workspace crates as path dependencies;
Cargo keeps feature-unified dependency identities consistent.

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(
        "conformance/probes/tlc-final-property-origins/models/TfBooleanFails/TfBooleanFails.tla",
    );
    let analysis = dive_sema::analyze_file(path, &dive_sema::Options::default())?;
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).map_err(|e| format!("{e:?}"))?);
    let cfg = dive_config::parse(&std::fs::read_to_string(path.with_extension("cfg"))?)
        .into_config().ok_or("invalid configuration")?;
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())?;
    assert_eq!(model.implied_temporal_action_names(), ["P", "P"]);
    let options = dive_check::CheckOptions { workers: 4, ..dive_check::CheckOptions::tlc(0, 0) };
    let report = dive_api::tlc_temporal::check_final(
        &ir, &cfg, &options, dive_liveness::phase_graph::Limits::default(),
    )?;
    if let dive_api::tlc_temporal::Outcome::Violation(v) = report.outcome() {
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
        assert_eq!(v.candidate().evidence.execution.expanded_property_names, ["P", "P"]);
        println!("configured={:?}; expanded={:?}", v.document().properties,
            v.candidate().evidence.execution.expanded_property_names);
    } else { return Err("expected validated violation".into()); }
    Ok(())
}
```
