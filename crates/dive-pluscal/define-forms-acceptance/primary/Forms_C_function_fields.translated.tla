---- MODULE Forms_C_function_fields ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL G==1 F[a \in {1,2}]==[G |-> a].G
 }
{ L:y:=F[1]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "7f145ffe" /\ chksum(tla) = "bf79206a")
VARIABLES pc, x, y

(* define statement *)
LOCAL G==1 F[a \in {1,2}]==[G |-> a].G


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F[1]
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
