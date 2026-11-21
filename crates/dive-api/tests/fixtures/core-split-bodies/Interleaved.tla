---- MODULE Interleaved ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Partial(i) == /\ i = 1
        /\ LET f == [n \in 0..1 |-> 1 \div n] IN x' = f[1]
Whole == [n \in 0..1 |-> x + n]
Total(i) == /\ x = i /\ x' = Whole[0]
Next == \E i \in 0..1 : Partial(i) \/ Total(i)
====
