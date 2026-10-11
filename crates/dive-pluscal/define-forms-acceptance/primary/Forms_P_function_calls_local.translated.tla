---- MODULE Forms_P_function_calls_local ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL G(a)==a+1 F[a \in {1,2}]==G(a)
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "21ce4322" /\ chksum(tla) = "a047fc88")
VARIABLES pc, x, y

(* define statement *)
LOCAL G(a)==a+1 F[a \in {1,2}]==G(a)


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
