---- MODULE P_plain_labels_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot Src)
(* --algorithm A variables g=0; process P\in {1,2} begin g:=1; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7401bcb8" /\ chksum(tla) = "9e8e781c")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Src1"]

Src1(self) == /\ pc[self] = "Src1"
              /\ g' = 1
              /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == Src1(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : SF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
