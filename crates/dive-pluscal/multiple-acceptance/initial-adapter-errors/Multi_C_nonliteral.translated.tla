---- MODULE Multi_C_nonliteral ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q \in1..g) variable y=0; { M: y:=2; F: g:=y; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "29e9d230" /\ chksum(tla) = "330159ba")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup (1..g)

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = [self \in 1..g |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self \in 1..g -> "M"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc[1] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

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

Next == P
           \/ (\E self \in 1..g: Q(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
