---- MODULE RawAxesStayRaw ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET s == {x, TRUE}
           p == ({} \X s) \X {x}
           c == [edge |-> p]
       IN /\ IsFiniteSet(c.edge)
          /\ x \in s
====
