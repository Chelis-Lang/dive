---- MODULE Proc_P_id_no_globals_default ----
EXTENDS Naturals, TLC
(* --algorithm A process P =1 variables x=0; begin L: x:=1; E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5f7c6062" /\ chksum(tla) = "739c9ebe")
VARIABLES pc, x

vars == << pc, x >>

ProcSet == {1}

Init == (* Process P *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ x' = x

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
