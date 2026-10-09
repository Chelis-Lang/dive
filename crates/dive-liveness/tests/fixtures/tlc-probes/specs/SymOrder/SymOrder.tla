---- MODULE SymOrder ----
\* Model values declared in the order p3, p1, p2 in the cfg, with a set-
\* valued variable and a variable recording the last process added. The
\* invariant fails on the second step; the printed trace shows which concrete
\* states TLC explored and printed.
EXTENDS Naturals, FiniteSets, TLC
CONSTANT Procs
VARIABLES s, last
Init == s = {} /\ last = "none"
Next == \E p \in Procs \ s : s' = s \cup {p} /\ last' = p
Perms == Permutations(Procs)
Inv == Cardinality(s) < 2
====
