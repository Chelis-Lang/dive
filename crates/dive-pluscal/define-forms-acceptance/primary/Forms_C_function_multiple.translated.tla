---- MODULE Forms_C_function_multiple ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}, b \in {2,3}] == a+b
 }
{ L:y:=F[1,2]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "8ba735b8" /\ chksum(tla) = "8e3a5eda")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}, b \in {2,3}] == a+b


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F[1,2]
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
