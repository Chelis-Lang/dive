---- MODULE Settle ----
\* Closed definitions too large to fold within the fold fuel (R9), which the
\* binder does not evaluate at start-up: the bound `n` of `SettleLib`'s
\* operators shares its name with a variable here, as `ZSequences`'s `ZSeq`
\* does under `LeastCircularSubstring`. `Corpus` (9,840 sequences) is
\* evaluated once when the core compiles its plan, so the invariant's
\* membership test reads a stored set. `Bad` fails when it is built, so its
\* body is left where it is used: unused (`Inv`) it raises nothing; used
\* (`InvBad`) it raises TLC's error there.
EXTENDS Naturals, SettleLib
VARIABLES s, n
Corpus == Seqs(0..2, 8)
Bad == Failing(100000)
Init == n = 0 /\ s \in {[i \in 0..1 |-> 0], [i \in 0..2 |-> 1]}
Next == /\ s' = IF s = <<>> THEN CHOOSE y \in Bad : TRUE
                ELSE [i \in DOMAIN s |-> (s[i] + 1) % 3]
        /\ UNCHANGED n
Inv == s \in Corpus
InvBad == s \in Corpus /\ s \notin Bad
====
