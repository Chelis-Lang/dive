---- MODULE Forms_P_function_constant_unicode-comment ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define \* comment λ🙂

 F[a \in {1,2}] == a+1
 end define;
begin L:y:=F[1]; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "5a5bdb72" /\ chksum(tla) = "2b5b5f14")
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

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
