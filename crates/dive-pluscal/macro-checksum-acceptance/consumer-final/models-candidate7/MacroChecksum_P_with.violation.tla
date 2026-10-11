---- MODULE MacroChecksum_P_with ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin with k=a do v:=k; end with; end macro; begin L: Put(x,1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "61a3c7e7" /\ chksum(tla) = "90f317b3")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET k == 1 IN
           x' = k
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

Good == x = 0
====
