# Graph conversion and normalization demand

Issue #756 records a distinction in the pinned CommunityModules overrides at
`78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3`. `AbstractGraphs.toGraph` checks that
`node` and `edge` convert to sets; this does not normalize an explicit
`SetEnumValue`. The separate `nodes` and `edges` helpers normalize them when the
caller actually needs their elements. `AreConnectedIn` checks node membership,
then self equality, before constructing adjacency. `IsStronglyConnected` returns
true for an empty node set before constructing adjacency. `SimplePath` always
constructs adjacency, including for an empty node set.

Consequently, the state-dependent `SelfDemand`, `EmptyDemand`, `AbsentDemand` and
`UndirectedSelf` controls succeed despite incomparable, unused edge elements.
`SkippedMalformed` demands its edges but their wrong tuple arities add no arc.
All five have 3 generated and distinct states, depth 3 and exit 0. `SelfViolation`
and `EmptyViolation` preserve the graph short circuit and violate `Inv` at x=2,
with the same counts/depth and exit 12. These seven model goldens and all twelve
accepted SANY module goldens come only from the pinned harness.

Five configurations remain source/API negative controls, with `models: []` in
their module manifests. The official generator refused their context-only
initial-error streams: `DemandedEdges`, `DemandedEndpoint`, `FieldPriority` and
`SimpleEmpty` have outer codes 2102/2103; `SelfEdgeType` has 1000/2103. Each exits
75. They have no model golden and supply no agreement credit. Their nested
messages demonstrate demanded comparisons or an invalid edge field. In
`FieldPriority`, formatting the invalid graph argument itself demands the
incomparable node set and raises the nested override error before the intended
argument diagnostic can be delivered. A type check cannot be used as permission
to bypass that diagnostic demand.

Regenerate the classified selection through the official harness, retaining raw
invocations outside the repository:

```sh
target/debug/dive-conformance golden generate --corpus graph-demand --tier all \
  --workers 1 --jobs 2 --timeout 30 --xmx 512m \
  --reason 'Refresh graph demand controls against the pinned oracle'
```

The complete first twelve-model attempted selection and its five refused
classifications remain in the external evidence archive. Established corpus
identities, populations and historical census results are unchanged.
