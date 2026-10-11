---- MODULE P_with_nested ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:with a=1,b=2 do with c=3,d=4 do x:=a+b+c+d; end with; end with; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "84ed8f39" /\ chksum(tla) = "b7bc900d")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
           LET b == 2 IN
              LET c == 3 IN
                 LET d == 4 IN
                    x' = a + b + c + d
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
