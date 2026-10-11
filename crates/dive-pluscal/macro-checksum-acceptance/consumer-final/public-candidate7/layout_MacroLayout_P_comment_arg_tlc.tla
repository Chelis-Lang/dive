---- MODULE MacroLayout_P_comment_arg ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,1 (* inline *) + 2); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4f4af27b" /\ chksum(tla) = "dd6b6ad5")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1 + 2
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
