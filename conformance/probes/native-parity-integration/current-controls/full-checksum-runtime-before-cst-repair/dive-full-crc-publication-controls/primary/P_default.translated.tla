---- MODULE P_default ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x;
begin L: x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "345ecc24" /\ chksum(tla) = "f3ecf2a3")
CONSTANT defaultInitValue
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = defaultInitValue
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
