---- MODULE Forms_P_function_constant_fair ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--fair algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}] == a+1
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "523c0734")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}] == a+1


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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
