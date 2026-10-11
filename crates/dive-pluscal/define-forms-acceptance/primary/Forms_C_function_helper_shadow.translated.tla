---- MODULE Forms_C_function_helper_shadow ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F==1 G[F \in {1,2}]==F
 }
{ L:y:=G[1]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "42f04965" /\ chksum(tla) = "c915e8db")
VARIABLES pc, x, y

(* define statement *)
LOCAL F==1 G[F \in {1,2}]==F


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = G[1]
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
