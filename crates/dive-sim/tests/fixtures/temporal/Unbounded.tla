---- MODULE Unbounded ----
\* x grows without bound; every finite trace is truncated by -depth.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Spec == Init /\ [][Next]_x
FairSpec == Spec /\ WF_x(Next)
EventuallyTen == <>(x = 10)
InfOftenEven == []<>(x % 2 = 0)
LeadsToTen == (x = 1) ~> (x = 10)
AlwaysBelowFour == [](x < 4)
AlwaysBelowTen == [](x < 10)
BelowTen == x < 10
StepIsIncrement == [][x' = x + 1]_x
StepIsDouble == [][x' = 2 * x]_x
InitZero == x = 0
InitOne == x = 1
StepBelowThree == [][x' < 3]_x
BelowThree == x < 3
BelowFour == x < 4
AliasDef == [x |-> x, double |-> 2 * x]
====
