---- MODULE FairInteraction_P_wrapped ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P \in{1,2} variable x=0; begin VeryLongFirstFairnessActionLabel: + x:=1; VeryLongLastFairnessActionLabel: - g:=x; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f4716902" /\ chksum(tla) = "fd9ede38")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "VeryLongFirstFairnessActionLabel"]

VeryLongFirstFairnessActionLabel(self) == /\ pc[self] = "VeryLongFirstFairnessActionLabel"
                                          /\ x' = [x EXCEPT ![self] = 1]
                                          /\ pc' = [pc EXCEPT ![self] = "VeryLongLastFairnessActionLabel"]
                                          /\ g' = g

VeryLongLastFairnessActionLabel(self) == /\ pc[self] = "VeryLongLastFairnessActionLabel"
                                         /\ g' = x[self]
                                         /\ pc' = [pc EXCEPT ![self] = "Done"]
                                         /\ x' = x

P(self) == VeryLongFirstFairnessActionLabel(self)
              \/ VeryLongLastFairnessActionLabel(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : /\ WF_vars((pc[self] # "VeryLongLastFairnessActionLabel") /\ P(self))
                               /\ SF_vars(VeryLongFirstFairnessActionLabel(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
