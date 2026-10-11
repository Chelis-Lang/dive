---- MODULE Forms_P_function_record ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==[value |-> a]
 end define;
begin L:y:=F[1].value; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "9ede4e4a" /\ chksum(tla) = "7648947a")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==[value |-> a]


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F[1].value
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
