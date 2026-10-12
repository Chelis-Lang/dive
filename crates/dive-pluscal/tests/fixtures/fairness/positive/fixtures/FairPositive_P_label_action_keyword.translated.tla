---- MODULE FairPositive_P_label_action_keyword ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; fair process P \in{1,2} variables x=0; begin WF: + x:=1; SF: - g:=x; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7cca99c7" /\ chksum(tla) = "75e199fa")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "WF"]

WF(self) == /\ pc[self] = "WF"
            /\ x' = [x EXCEPT ![self] = 1]
            /\ pc' = [pc EXCEPT ![self] = "SF"]
            /\ g' = g

SF(self) == /\ pc[self] = "SF"
            /\ g' = x[self]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ x' = x

P(self) == WF(self) \/ SF(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : WF_vars((pc[self] # "SF") /\ P(self)) /\ SF_vars(WF(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
