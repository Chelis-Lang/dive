---- MODULE CallReads ----
EXTENDS Naturals, TLC
VARIABLES x, y

\* Recursive definitions whose bodies read a variable that is not an argument.
RECURSIVE RecY(_)
RecY(n) == IF n = 0 THEN y ELSE RecY(n - 1)
f[n \in 0..2] == IF n = 0 THEN x ELSE f[n - 1]

RECURSIVE Shown(_)
Shown(n) == IF n = 0 THEN PrintT(<<"x", x>>) ELSE Shown(n - 1)

Init == x = 0 /\ y = 0
Next == \/ x < 2 /\ x' = x + 1 /\ UNCHANGED y
        \/ y < 3 /\ y' = y + 1 /\ UNCHANGED x

InvY == RecY(2) < 3
InvX == f[2] < 3
InvShown == Shown(1)
====
