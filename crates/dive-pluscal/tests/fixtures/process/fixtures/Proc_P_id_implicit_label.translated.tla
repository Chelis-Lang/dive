---- MODULE Proc_P_id_implicit_label ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin x:=1; g:=x; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "da1524cd" /\ chksum(tla) = "cbef7db2")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ x' = 1
         /\ g' = x'
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Lbl_1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
