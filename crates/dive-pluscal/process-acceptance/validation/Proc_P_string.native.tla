---- MODULE Proc_P_string ----
EXTENDS Naturals,TLC
(* --algorithm A process P ="p" begin L: skip; E: skip; end process; end algorithm; *)

VARIABLE pc

vars == << pc >>

ProcSet == {"p"}

Init == /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc["p"] = "L"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["p"] = "E"]

E == /\ pc["p"] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["p"] = "Done"]

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
