---- MODULE LaterDivDiagnosticOverflow ----
EXTENDS Integers,TLC
VARIABLES x,y
Init == x=0 /\ y=0
Next == x<3 /\ x' \in {x+1,x+2} /\ y'=IF x>=1 /\ x'=x+2 THEN (2147483647+1) ELSE 0
Prop == [][x<1 \/ (1 \div 0)=0]_<<x,y>>
====
