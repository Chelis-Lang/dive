---- MODULE MacroLayout_P_multiline_body ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=IF TRUE THEN 1
 ELSE 2; end macro; begin L:Put(x,1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ad80fbc1" /\ chksum(tla) = "e316d696")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                                       IF TRUE THEN 1
             ELSE 2
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
