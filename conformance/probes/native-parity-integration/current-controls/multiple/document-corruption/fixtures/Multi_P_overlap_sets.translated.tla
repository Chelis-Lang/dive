---- MODULE Multi_P_overlap_sets ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=1; E: g:=x; end process;  process Q \in{2,3} variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a91b929b" /\ chksum(tla) = "1a58981f")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == ({1,2}) \cup ({2,3})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        (* Process Q *)
        /\ y = [self \in {2,3} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self \in {2,3} -> "M"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << g, y >>

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y >>

P(self) == L(self) \/ E(self)

M(self) == /\ pc[self] = "M"
           /\ y' = [y EXCEPT ![self] = 2]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << g, x >>

F(self) == /\ pc[self] = "F"
           /\ g' = y[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y >>

Q(self) == M(self) \/ F(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ (\E self \in {2,3}: Q(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
