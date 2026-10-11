---- MODULE Forms_P_function_quantifier ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==IF \E q \in {1,2}:q=a THEN a ELSE 0
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "7ac9d41c" /\ chksum(tla) = "3204dbc6")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==IF \E q \in {1,2}:q=a THEN a ELSE 0


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
