---- MODULE P_fair_self ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P \in {1,2} variables z=self; begin L:g:=z; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "1b180190")
VARIABLES pc, g, z

vars == << pc, g, z >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z = [self \in {1,2} |-> self]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = z[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ z' = z

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, z >>

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
