---- MODULE Forms_C_function_bound_recursive ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in DOMAIN F]==a
 }
{ L:y:=0; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "55e67943" /\ chksum(tla) = "a07836e4")
VARIABLES pc, x, y

(* define statement *)
F[a \in DOMAIN F]==a


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
