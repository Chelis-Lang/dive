---- MODULE InfCup ----
\* Unions with an infinite side stay lazy, as TLC's SetCupValue and
\* UnionValue: membership tests each side in turn (Disruptor's TypeOk).
EXTENDS Integers, Sequences
CONSTANT NULL
VARIABLES x, f
Vals == Int \cup {NULL}
Init == x = NULL /\ f = [i \in 0..1 |-> NULL]
Next == /\ x' \in {NULL, -1, 2}
        /\ f' \in {[f EXCEPT ![0] = x], [f EXCEPT ![1] = 3]}
Inv == /\ x \in Vals
       /\ x \in {NULL} \cup Nat \/ x = -1
       /\ f \in [0..1 -> Vals]
       /\ f \in UNION { [0..1 -> Vals] }
       /\ f \in [0..1 -> Int \cup {NULL}] \cup [a : {1}]
       /\ x \in UNION { {NULL}, {-1} } \cup Nat
       /\ <<x>> \in Seq(Vals)
====
