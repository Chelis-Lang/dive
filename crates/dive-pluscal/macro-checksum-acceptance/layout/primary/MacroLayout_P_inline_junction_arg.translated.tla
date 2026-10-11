---- MODULE MacroLayout_P_inline_junction_arg ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,TRUE /\ FALSE); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1ee2ebea" /\ chksum(tla) = "1f6da274")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (TRUE /\ FALSE)
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
