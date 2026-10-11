---- MODULE P_local_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P \in {1,2} variables z\in {0,1}; begin L:g:=z; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f19eb617" /\ chksum(tla) = "b6ee4368")
VARIABLES pc, g, z

vars == << pc, g, z >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z \in [{1,2} -> {0,1}]
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

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
