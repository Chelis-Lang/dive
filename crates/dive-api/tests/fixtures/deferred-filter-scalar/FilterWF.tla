---- MODULE FilterWF ----
EXTENDS Naturals, Sequences
CONSTANT Max
VARIABLE x
LimitedSeq(S) == UNION {[1..n -> S] : n \in 1..Max}
Init == x = FALSE /\ Seq({FALSE}) = {<<FALSE>>}
Pick == \E f \in {g \in [{1} -> BOOLEAN] : g[1] = TRUE} : x' = f[1]
Spec == Init /\ [][Pick]_x /\ WF_x(Pick)
Hold == <>x
Fail == []<>~x
====
