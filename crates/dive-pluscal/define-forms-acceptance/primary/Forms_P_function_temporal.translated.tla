---- MODULE Forms_P_function_temporal ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==[] (x=a)
 end define;
begin L:y:=0; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "bb82b540" /\ chksum(tla) = "dc4838d8")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==[] (x=a)


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
