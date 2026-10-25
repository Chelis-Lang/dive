# Owned Init exception/replay messages

These three small original stdout fixtures are read in full by the pure
`tlc::result::init_replay_tests` controls. They were produced by the normal
pinned TLC `94d0c5070852b37b801bb8998c436383ab7ee37b` harness at source
`4a9b1d3b8cefed628b94a4d83d813b1a3e66b402`, using the independent
`nested-powerset-primary` corpus. `facts.json` pins whole fixture bytes,
normal uncensored physical exits and source inputs. TLC output is MIT-licensed.

The explicit Cardinality stream has GENERAL owning NONE2154 owning NONE2178.
The non-Boolean stream has GENERAL owning NONE2215 and prints the accepted empty
set in both the initial pass and diagnostic replay. Both later report2103 and
physically exit75. The deciding class remains GENERAL; child codes authenticate
only the bounded Init-replay route. The required-component stream instead
reports ERROR2147 after GENERAL and must retain its existing classification.

These files are classifier inputs, not generated goldens. No parser result or
new nested-powerset implementation is asserted by retaining them. Unknown or
bare-text exceptions (including issue #436) remain outside issue #473's scope.
