---- MODULE Forms_P_local_calls_function ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==a+1 LOCAL G(a)==F[a]
 end define;
begin L:y:=G(1); E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "901e4dd9" /\ chksum(tla) = "525aa856")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==a+1 LOCAL G(a)==F[a]


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = G(1)
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
