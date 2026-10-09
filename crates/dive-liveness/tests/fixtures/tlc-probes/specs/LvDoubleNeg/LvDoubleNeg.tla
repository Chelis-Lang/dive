---- MODULE LvDoubleNeg ----
\* []~~[](x < 3): upstream 4260e4712 fixed spurious counterexamples from
\* unnormalized temporal negations. The property holds; the pinned TLC must
\* report success.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == []~~[](x < 3)
====
