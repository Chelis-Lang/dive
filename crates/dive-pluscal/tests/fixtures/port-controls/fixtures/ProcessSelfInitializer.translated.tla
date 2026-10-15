---- MODULE ProcessSelfInitializer ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A process P =1 variables x=self; begin L: skip; end process; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "19aff19" /\ chksum(tla) = "87e8d811")
VARIABLES pc, x

vars == << pc, x >>

ProcSet == {1}

Init == (* Process P *)
        /\ x = self
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ x' = x

P == L

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
