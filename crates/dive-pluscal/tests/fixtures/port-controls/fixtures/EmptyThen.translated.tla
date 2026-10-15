---- MODULE EmptyThen ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; begin L:if x=0 then else x:=1; end if; E:skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "8190097c" /\ chksum(tla) = "515a60f2")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = x
           ELSE /\ x' = 1
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
