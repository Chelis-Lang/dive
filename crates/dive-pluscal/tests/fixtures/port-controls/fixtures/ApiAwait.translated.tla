---- MODULE ApiAwait ----
EXTENDS Integers,TLC
(*
--algorithm A
variable x=0; begin L: await /\ TRUE
 /\ x=0; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "6dbbda3b" /\ chksum(tla) = "314b4f0")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\                             /\ TRUE
        /\ x=0
     /\ pc' = "E"
     /\ x' = x

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
