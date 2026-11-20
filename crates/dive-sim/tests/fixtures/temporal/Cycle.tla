---- MODULE Cycle ----
\* x cycles 0 -> 1 -> 2 -> 0; every trace closes a loop after 3 steps.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
FairSpec == Spec /\ WF_x(Next)
EventuallyFive == <>(x = 5)
InfOftenFive == []<>(x = 5)
LeadsToFive == (x = 1) ~> (x = 5)
InfOftenZero == []<>(x = 0)
AlwaysSmall == [](x < 3)
AlwaysSmallFalse == [](x < 2)
====
