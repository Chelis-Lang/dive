---- MODULE SymViewSets ----
\* VIEW with SYMMETRY over sets and sequences of model values, to a
\* deadlock: the VIEW keeps the log's first element, which the image fixes.
EXTENDS Naturals, Sequences, TLC
CONSTANT P
VARIABLE s, log
Init == s = {} /\ log = <<>>
Next == \E p \in P : /\ p \notin s
                     /\ s' = s \cup {p}
                     /\ log' = Append(log, p)
Sym == Permutations(P)
View == <<s, IF log = <<>> THEN log ELSE <<Head(log)>>>>
====
