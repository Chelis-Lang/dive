---- MODULE Multi_P_setid ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=1; E: g:=x; end process;  process Q =3 variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "18d851a5" /\ chksum(tla) = "524dcd8")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == ({1,2}) \cup {3}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        (* Process Q *)
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self = 3 -> "M"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << g, y >>

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y >>

P(self) == L(self) \/ E(self)

M == /\ pc[3] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![3] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc[3] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![3] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q
           \/ (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
