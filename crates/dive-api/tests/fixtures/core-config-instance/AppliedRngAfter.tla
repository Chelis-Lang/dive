---- MODULE AppliedRngAfter ----
EXTENDS Naturals, TLC
CONSTANTS C(_), N
VARIABLE x
R == RandomElement(1..N)
Target(n) == PrintT(<<n,x>>) /\ n=x
Init == x=R
Next == x'=R
Inv == x=R /\ C(R)
====
