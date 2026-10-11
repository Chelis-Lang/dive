---- MODULE P_local_dependent ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P \in {1,2} variables z=0,w\in {z}; begin L:g:=w; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d5890a53" /\ chksum(tla) = "b7c86749")
VARIABLES pc, g, z, w

vars == << pc, g, z, w >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z = [self \in {1,2} |-> 0]
        /\ w \in [{1,2} -> {z[CHOOSE self \in  {1,2} : TRUE]}]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = w[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << z, w >>

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, z, w >>

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
