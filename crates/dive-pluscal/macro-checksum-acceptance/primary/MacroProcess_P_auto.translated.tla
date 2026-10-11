---- MODULE MacroProcess_P_auto ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro; process P\in {1,2} variable z=0; begin Put(z,self); skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "fb2e719c" /\ chksum(tla) = "4c39459")
VARIABLES pc, x, y, z

vars == << pc, x, y, z >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        (* Process P *)
        /\ z = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_1(self) == /\ pc[self] = "Lbl_1"
               /\ z' = [z EXCEPT ![self] = self]
               /\ TRUE
               /\ pc' = [pc EXCEPT ![self] = "Done"]
               /\ UNCHANGED << x, y >>

P(self) == Lbl_1(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
