---- MODULE UnionOne ----
\* `UNION {S}` for a set constructor `S` is `S`, kept lazy (04-plan-compiler.md
\* section 4.2). The invariant fails once f takes the value 3.
EXTENDS Naturals
R == {"a", "b"}
VARIABLES f, r
Init == f = [i \in 0..2 |-> 0] /\ r = [a |-> {}]
Next == \E i \in 0..2, v \in 0..3 :
          /\ f' = [f EXCEPT ![i] = v]
          /\ r' = [a |-> IF v = 0 THEN {} ELSE {"a"}]
Inv == /\ r \in UNION { [a : SUBSET R] }
       /\ <<f[0], f[1]>> \in UNION { (0..3) \X (0..3) }
       /\ {f[0]} \in UNION { SUBSET (0..3) }
       /\ f \in UNION { [0..2 -> 0..2] }
====
