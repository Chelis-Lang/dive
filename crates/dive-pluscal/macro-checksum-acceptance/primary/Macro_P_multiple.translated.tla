---- MODULE Macro_P_multiple ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; process P=1 variable z=0; begin L: Put(z,1); E: skip; end process; process Q \in {2,3} variable w=0; begin M: Put(w,self); F: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "48ad1bbd" /\ chksum(tla) = "c5b00722")
VARIABLES pc, x, y, z, w

vars == << pc, x, y, z, w >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        (* Process P *)
        /\ z = 0
        (* Process Q *)
        /\ w = [self \in {2,3} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self \in {2,3} -> "M"]

L == /\ pc[1] = "L"
     /\ z' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << x, y, w >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y, z, w >>

P == L \/ E

M(self) == /\ pc[self] = "M"
           /\ w' = [w EXCEPT ![self] = self]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, y, z >>

F(self) == /\ pc[self] = "F"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y, z, w >>

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
