---- MODULE C_plain_root_last_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot One, -labelRoot Two, -label)
(* --algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "2d8c31bf" /\ chksum(tla) = "a22a219")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Two1"]

Two1(self) == /\ pc[self] = "Two1"
              /\ g' = 1
              /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == Two1(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
