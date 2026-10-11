---- MODULE P_two_members ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P \in {1,2} begin L:g:=g+1; E:skip; end process; fair process Q \in {3,4} begin M:g:=g+1; F:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f13e3d9c" /\ chksum(tla) = "57a1d66d")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2}) \cup ({3,4})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self \in {3,4} -> "M"]

L(self) == /\ pc[self] = "L"
           /\ g' = g+1
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

M(self) == /\ pc[self] = "M"
           /\ g' = g+1
           /\ pc' = [pc EXCEPT ![self] = "F"]

F(self) == /\ pc[self] = "F"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

Q(self) == M(self) \/ F(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {3,4}: Q(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {3,4} : WF_vars(Q(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
