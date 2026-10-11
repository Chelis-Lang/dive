---- MODULE Forms_P_function_bound_dependency ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL S=={1,2} F[a \in S]==a
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "a57f809c" /\ chksum(tla) = "75f42afd")
VARIABLES pc, x, y

(* define statement *)
LOCAL S=={1,2} F[a \in S]==a


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
