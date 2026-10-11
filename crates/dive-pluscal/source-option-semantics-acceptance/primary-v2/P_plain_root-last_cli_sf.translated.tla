---- MODULE P_plain_root_last_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot One, -labelRoot Two, -label)
(* --algorithm A variables g=0; process P\in {1,2} begin g:=1; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "93159654" /\ chksum(tla) = "4ba3d83d")
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

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : SF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
