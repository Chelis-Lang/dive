---- MODULE Forms_C_function_init_conflict ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 Init[a \in {1,2}]==a
 }
{ L:y:=0; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "f6d8e03d" /\ chksum(tla) = "370acd45")
VARIABLES pc, x, y

(* define statement *)
Init[a \in {1,2}]==a


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = 0
     /\ pc' = "E"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
