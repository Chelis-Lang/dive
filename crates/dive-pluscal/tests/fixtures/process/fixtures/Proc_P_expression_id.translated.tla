---- MODULE Proc_P_expression_id ----
EXTENDS Naturals,TLC
(* --algorithm A process P =1+1 begin L: skip; E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "2a6e63dc" /\ chksum(tla) = "1756657a")
VARIABLE pc

vars == << pc >>

ProcSet == {1+1}

Init == /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1+1] = "L"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1+1] = "E"]

E == /\ pc[1+1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1+1] = "Done"]

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
