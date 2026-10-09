# Symmetry closure kernel

`DiveSymmetryGroup.java` is an original MIT driver calling the pinned
`MVPerms.permutationSubgroup` directly. The 1,329 inputs cover all ordered pairs
of total maps on two and three values, all ordered pairs of permutations on
four values, disjoint and partial generators, duplicates, identities, a
five-value symmetric group and a six-value cycle. `tlc.jsonl` is its raw output;
no closure implementation in the driver computes expected answers.

The set wrapper deliberately retains the input generator enumeration order.
This tests closure and insertion order, not set normalization, model binding,
warnings or reduction. The Rust controls match all images in order in TLC mode,
and check every valid bijective case in checked mode. Invalid checked cases
must refuse, including maps that collide with omitted identities.

Reproduce with a Temurin 17 JDK (`DIVE_JDK`) and the pinned cache:

```sh
DIVE_SYM_FIXTURES=crates/dive-model/tests/fixtures/symmetry-group
DIVE_SYM_JAR="$DIVE_CACHE_DIR/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar"
mkdir -p target/symmetry-java
"$DIVE_JDK/bin/javac" --release 17 -g:none -cp "$DIVE_SYM_JAR" \
  -d target/symmetry-java "$DIVE_SYM_FIXTURES/DiveSymmetryGroup.java"
"$DIVE_JDK/bin/java" -cp "target/symmetry-java:$DIVE_SYM_JAR" \
  DiveSymmetryGroup "$DIVE_SYM_FIXTURES/inputs.txt" > target/symmetry-java/tlc.jsonl
cmp "$DIVE_SYM_FIXTURES/tlc.jsonl" target/symmetry-java/tlc.jsonl
cargo test -p dive-model --test symmetry --locked
```

Oracle: `tlc-2026.10.06.014338`, revision
`94d0c5070852b37b801bb8998c436383ab7ee37b`, jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
Captured 2026-10-09 UTC using Temurin 17.0.20.1+1.

| File | SHA-256 |
|---|---|
| DiveSymmetryGroup.java | f0e1300ad82976f4650251d3cc8af40fc8a4e6aa8fa04e77c58a4b66fa3c6b07 |
| inputs.txt | 11174ab7a381a707daded7cda951f0faf10963941b090a86e57920581539eb05 |
| tlc.jsonl | 659ec069a70af6c80e3fa41ba2b091f7b7ad24117082b1fea63df970fd3e37f2 |

## Empty declarations

The direct Java kernel throws `IllegalArgumentException` for zero or one
supplied function (`util.Set` receives capacity `size - 1`). The Rust kernel
returns `TooFewGenerators`. TLC's binding often disables such cases first,
with warning 2300; binding is a separate, still-open part of S1.

`SyEmpty` is the minimal model showing the empty-set failure through TLC's
normal CLI: `Sym == {}`, `SYMMETRY Sym`, one boolean stuttering state. Its raw
oracle run exits 255 with GENERAL and an `IllegalArgumentException` stack.
SANY accepts it. Capture it through the harness, without editing transcripts:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 --specs "$DIVE_SYM_FIXTURES/specs" \
  --out "$DIVE_SYM_FIXTURES" --timeout 30 SyEmpty
```

These are raw kernel/model fixtures, not edited goldens or evidence that dive
can reduce states. Checking `SYMMETRY` models remains a named refusal.

## Binding evidence for the next S1 stage

These nine raw model captures complement SyEmpty. They pin the normal CLI,
not a claim that the Rust binding path implements them yet:

| Model | Pinned TLC outcome |
|---|---|
| SyMissing | exit 151, code 2229, undefined symmetry function |
| SyConstant | exit 255, GENERAL, named constant is not an operator |
| SyNotSet | exit 255, GENERAL, result is not a set of functions |
| SyTuple | exit 255, GENERAL, elements are not function records |
| SyNonModel | exit 255, GENERAL, domain/range are not model values |
| SySmall | success, warning 2300 for singleton S |
| SyUnused | success, warning 2300 for unmoved singleton B |
| SyNonBiject | success: 3 generated, 1 distinct, depth 1 |
| SyNonBijectNone | invariant violation: 3 generated, 2 distinct, depth 2 |

Reproduce with the same dev-capture command and these nine names instead of
SyEmpty. SyNonBiject declares a collapsing map plus identity through a helper
operator (so the syntactic small-set heuristic does not disable reduction).
Its unreduced control finds the real violation `x = b`, proving a missed
safety error rather than just a change in counts. Exact lookup/override order,
warning construction and canonical keys remain tracked in issue #104.
