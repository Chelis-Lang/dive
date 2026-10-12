---- MODULE Proc_P_set_partial_labels_label ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0; begin x:=1; E: g:=x; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f5fba25e" /\ chksum(tla) = "217ad404")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_1(self) == /\ pc[self] = "Lbl_1"
               /\ x' = [x EXCEPT ![self] = 1]
               /\ pc' = [pc EXCEPT ![self] = "E"]
               /\ g' = g

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ x' = x

P(self) == Lbl_1(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
