---- MODULE SymSets ----
\* SYMMETRY over sets and tuples of model values, to a deadlock.
EXTENDS Naturals, Sequences, TLC
CONSTANT P
VARIABLE s, log
Init == s = {} /\ log = <<>>
Next == \E p \in P : /\ p \notin s
                     /\ s' = s \cup {p}
                     /\ log' = Append(log, p)
Sym == Permutations(P)
====
