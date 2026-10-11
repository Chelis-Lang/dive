---- MODULE Forms_P_local_recursive ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 RECURSIVE F(_) LOCAL F(a)==IF a=0 THEN 0 ELSE F(a-1)
 end define;
begin L:y:=F(2); E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "625294e7" /\ chksum(tla) = "b3f5d843")
VARIABLES pc, x, y

(* define statement *)
RECURSIVE F(_) LOCAL F(a)==IF a=0 THEN 0 ELSE F(a-1)


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F(2)
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
