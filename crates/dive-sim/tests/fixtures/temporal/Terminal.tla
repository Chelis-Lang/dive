---- MODULE Terminal ----
\* x counts to 2 and then has no successor: the behavior stutters forever.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Spec == Init /\ [][Next]_x
FairSpec == Spec /\ WF_x(Next)
EventuallyTwo == <>(x = 2)
EventuallyFive == <>(x = 5)
InfOftenZero == []<>(x = 0)
EventuallyAlwaysTwo == <>[](x = 2)
====
