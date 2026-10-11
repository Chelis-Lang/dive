---- MODULE MacroLayout_P_unicode_comment ----
EXTENDS Integers,TLC
\* λ😀 outside quoted tokens
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "40fe0264" /\ chksum(tla) = "14608fc6")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
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
