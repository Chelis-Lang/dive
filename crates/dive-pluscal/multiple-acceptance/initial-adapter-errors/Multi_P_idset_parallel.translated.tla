---- MODULE Multi_P_idset_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: x:=1 || g:=2; E: skip; end process;  process Q \in{2,3} variable y=0; begin M: y:=2 || g:=1; F: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f9e08fbc" /\ chksum(tla) = "7febefc8")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = [self \in {2,3} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self \in {2,3} -> "M"]

L == /\ pc[1] = "L"
     /\ /\ g' = 2
        /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ y' = y

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, x, y >>

P == L \/ E

M(self) == /\ pc[self] = "M"
           /\ /\ g' = 1
              /\ y' = [y EXCEPT ![self] = 2]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

F(self) == /\ pc[self] = "F"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, x, y >>

Q(self) == M(self) \/ F(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ (\E self \in {2,3}: Q(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
