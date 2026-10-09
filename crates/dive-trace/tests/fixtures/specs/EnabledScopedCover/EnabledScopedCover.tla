---- MODULE EnabledScopedCover ----
EXTENDS Integers
VARIABLES x,y
Init == x \in (-1)..4 /\ y \in (-1)..4
Next == UNCHANGED <<x,y>>
Relation(a,b) == x' \in {a,b} /\ y' \in 0..1 /\ x'+y'=x+y
Wrap(pair) == Relation(pair[1],pair[2])
R01 == Wrap(<<0,1>>)
R12 == Wrap(<<1,2>>)
Union == Relation(0,1) \/ Relation(1,2)
Checks == /\ (ENABLED R01) <=> (x+y >= 0 /\ x+y <= 2)
          /\ (ENABLED R12) <=> (x+y >= 1 /\ x+y <= 3)
          /\ (ENABLED Union) <=> (x+y >= 0 /\ x+y <= 3)
====
