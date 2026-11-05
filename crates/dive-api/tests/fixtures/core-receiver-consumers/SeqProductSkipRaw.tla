---- MODULE SeqProductSkipRaw ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == LET raw == {x, TRUE} IN <<>> \in Seq(raw \X {})
====
