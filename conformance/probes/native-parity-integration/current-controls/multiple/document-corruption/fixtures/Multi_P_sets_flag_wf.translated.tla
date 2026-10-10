---- MODULE Multi_P_sets_flag_wf ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=1; E: g:=x; end process;  process Q \in{3,4} variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5a34f23c" /\ chksum(tla) = "2d9b3046")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == ({1,2}) \cup ({3,4})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        (* Process Q *)
        /\ y = [self \in {3,4} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self \in {3,4} -> "M"]

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
           \/ (\E self \in {3,4}: Q(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : WF_vars(P(self))
        /\ \A self \in {3,4} : WF_vars(Q(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
