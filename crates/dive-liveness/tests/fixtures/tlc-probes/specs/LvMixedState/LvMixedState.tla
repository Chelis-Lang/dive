---- MODULE LvMixedState ----
\* PROPERTY Prop == [](x < 2) /\ <>(x = 9). The []P conjunct is a state
\* predicate TLC checks as an invariant during the search; <>(x = 9) is
\* checked by liveness. Both are violated. Which is reported, with what
\* message, and with what counts?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == [](x < 2) /\ <>(x = 9)
====
