---- MODULE Define_P_no_definition ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 x + 1
 end define;
begin L: x := 1; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "3723740f" /\ chksum(tla) = "8644c639")
VARIABLES pc, x, y

(* define statement *)
x + 1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"
     /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
