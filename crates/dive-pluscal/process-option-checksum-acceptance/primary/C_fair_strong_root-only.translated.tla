---- MODULE C_fair_strong_root_only ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; fair+ process(P \in {1,2}){L:g:=1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "367d7730")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = 1
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)
        /\ \A self \in {1,2} : SF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
