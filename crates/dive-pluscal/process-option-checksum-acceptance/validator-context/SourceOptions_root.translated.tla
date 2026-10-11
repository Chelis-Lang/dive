---- MODULE SourceOptions_root ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot, X_)
(* --algorithm A variables g=0; process P \in {1,2} begin g:=1; g:=2; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "666e5e10" /\ chksum(tla) = "a53c3a82")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "X_1"]

X_1(self) == /\ pc[self] = "X_1"
             /\ g' = 1
             /\ pc' = [pc EXCEPT ![self] = "X_2"]

X_2(self) == /\ pc[self] = "X_2"
             /\ g' = 2
             /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == X_1(self) \/ X_2(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
