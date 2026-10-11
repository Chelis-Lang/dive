---- MODULE Forms_C_function_case ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}]==CASE a=1 -> 2 [] OTHER -> 0
 }
{ L:y:=F[1]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "5361ffde" /\ chksum(tla) = "668eee95")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==CASE a=1 -> 2 [] OTHER -> 0


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
