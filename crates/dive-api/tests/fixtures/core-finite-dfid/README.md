# Eager DFID compound candidate lifetimes

`NextEnumerated` and `InitEnumerated` construct a singleton set inside an
assignment enumeration continuation. The vector must retain the original local
value until later constraints, key normalization and property checks demand it;
returning from the generator continuation must not reclaim that value.
`NextViolation` checks the same path with a concrete independently validated
invariant word: `x=0,s={0}` followed by `x=2,s={2}`.

`oracle.json` is a compact parsed summary generated from the pinned TLC tool
stream, with input hashes, pin and reproduction command. It is read by the API
regression test. `level` is the last started iterative level, not a BFS diameter.
Worker scheduling and TLC's fresh per-iteration selection seed are not compared.
The tests also compare both native engines/profiles/stores against Reference;
private kernel controls cover disabled hashcons and repeated-iteration transient
reclamation. Raw tool streams remain outside the repository.
