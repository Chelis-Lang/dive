---- MODULE Proc_C_string ----
EXTENDS Naturals,TLC
(* --algorithm A { process (P ="p") { L: skip; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "53dd4ffa" /\ chksum(tla) = "1a37012c")
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

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
