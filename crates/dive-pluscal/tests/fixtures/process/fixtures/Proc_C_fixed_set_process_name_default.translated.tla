---- MODULE Proc_C_fixed_set_process_name_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables P=0; process (P \in{1,2}) variables x=0;  { L: P:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "92d6be5" /\ chksum(tla) = "91e6d54c")
\* Process P at line 3 col 35 changed to P_
VARIABLES pc, P, x

vars == << pc, P, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ P = 0
        (* Process P_ *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ P' = 1
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ x' = x

P_(self) == L(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P_(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
