---- MODULE P_while ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: while x<2 do x:=x+1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "29bf749a" /\ chksum(tla) = "78d8cfd6")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ x' = x+1
                /\ pc' = "L"
           ELSE /\ pc' = "Done"
                /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
