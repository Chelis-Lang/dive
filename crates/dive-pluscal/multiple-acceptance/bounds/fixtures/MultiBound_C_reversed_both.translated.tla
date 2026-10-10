---- MODULE MultiBound_C_reversed_both ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; process (P \in3..1) variable x=0; { L: x:=1; E: g:=x; } process (Q \in4..2) variable y=0; { M: y:=2; F: g:=y; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "e1dba3b8" /\ chksum(tla) = "e370896b")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == (3..1) \cup (4..2)

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in 3..1 |-> 0]
        (* Process Q *)
        /\ y = [self \in 4..2 |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in 3..1 -> "L"
                                        [] self \in 4..2 -> "M"]

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

Next == (\E self \in 3..1: P(self))
           \/ (\E self \in 4..2: Q(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
