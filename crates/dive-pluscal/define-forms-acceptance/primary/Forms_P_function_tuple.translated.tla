---- MODULE Forms_P_function_tuple ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[<<a,b>> \in {<<1,2>>}] == a+b
 end define;
begin L:y:=F[<<1,2>>]; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "b4fe21a5" /\ chksum(tla) = "67c7c3e9")
VARIABLES pc, x, y

(* define statement *)
F[<<a,b>> \in {<<1,2>>}] == a+b


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F[<<1,2>>]
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
