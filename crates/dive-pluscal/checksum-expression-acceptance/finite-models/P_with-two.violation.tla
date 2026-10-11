---- MODULE P_with_two ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:with a=1,b=2 do x:=a+b; end with; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ca3672f2" /\ chksum(tla) = "8aae9aa1")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
           LET b == 2 IN
              x' = a + b
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

Good == pc # "E"
====
