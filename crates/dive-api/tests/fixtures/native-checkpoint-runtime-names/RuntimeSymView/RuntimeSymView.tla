---- MODULE RuntimeSymView ----
EXTENDS Naturals,TLC
CONSTANTS A,B
VARIABLE n,s,owner
Init == n=0 /\ s="0" /\ owner=B
Next == n<4 /\ n'=n+1 /\ s'=ToString(n+1) /\ owner'=owner
Inv == n<3
View == <<n,s>>
Perms == Permutations({A,B})
====
