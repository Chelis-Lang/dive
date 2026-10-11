---- MODULE Forms_P_function_duplicate ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==a F[a \in {1,2}]==a+1
 end define;
begin L:y:=0; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "9ce30b76" /\ chksum(tla) = "e89e8420")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==a F[a \in {1,2}]==a+1


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
