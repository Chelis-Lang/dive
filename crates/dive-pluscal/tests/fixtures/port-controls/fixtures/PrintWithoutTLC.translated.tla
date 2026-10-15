---- MODULE PrintWithoutTLC ----
(*
--algorithm A
variable x=0; begin L: print x; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "4fc7e373" /\ chksum(tla) = "1d7afdca")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ PrintT(x)
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
