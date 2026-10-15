---- MODULE Proc_P_set_label_name_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0; begin P: x:=1; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f8cc37bb" /\ chksum(tla) = "6e214db0")
\* Label P of process P at line 3 col 76 changed to P_
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "P_"]

P_(self) == /\ pc[self] = "P_"
            /\ x' = [x EXCEPT ![self] = 1]
            /\ pc' = [pc EXCEPT ![self] = "Done"]
            /\ g' = g

P(self) == P_(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
