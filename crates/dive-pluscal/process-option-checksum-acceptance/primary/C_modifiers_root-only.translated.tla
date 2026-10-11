---- MODULE C_modifiers_root_only ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair+ process(P \in {1,2}){L:-g:=1; M:+g:=2; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7b6c6eaa" /\ chksum(tla) = "68d5ccb8")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = 1
           /\ pc' = [pc EXCEPT ![self] = "M"]

M(self) == /\ pc[self] = "M"
           /\ g' = 2
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ M(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : SF_vars((pc[self] # "L") /\ P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
