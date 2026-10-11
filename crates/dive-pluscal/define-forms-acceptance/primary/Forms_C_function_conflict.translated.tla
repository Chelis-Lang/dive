---- MODULE Forms_C_function_conflict ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 x[a \in {1,2}]==a
 }
{ L:y:=0; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "33cc4c35" /\ chksum(tla) = "80a600b5")
VARIABLES pc, x, y

(* define statement *)
x[a \in {1,2}]==a


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
