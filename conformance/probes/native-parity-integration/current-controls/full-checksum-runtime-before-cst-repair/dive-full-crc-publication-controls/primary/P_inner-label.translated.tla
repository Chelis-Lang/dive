---- MODULE P_inner_label ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: while x<2 do B:x:=x+1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ee715f15" /\ chksum(tla) = "844e4cb2")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ pc' = "B"
           ELSE /\ pc' = "Done"
     /\ x' = x

B == /\ pc = "B"
     /\ x' = x+1
     /\ pc' = "L"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
