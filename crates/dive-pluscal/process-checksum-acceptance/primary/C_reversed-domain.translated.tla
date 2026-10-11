---- MODULE C_reversed_domain ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in 2..1){L:g:=g+1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "8bb82534" /\ chksum(tla) = "ea85a09d")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == (2..1)

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = g+1
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in 2..1: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
