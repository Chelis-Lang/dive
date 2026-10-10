---- MODULE Proc_P_id_assert_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: x:=1; assert x=0; E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7ed64558" /\ chksum(tla) = "ac9ca789")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ Assert(x'=0, "Failure of assertion at line 3, column 76.")
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, x >>

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
