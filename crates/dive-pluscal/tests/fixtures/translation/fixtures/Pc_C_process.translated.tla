---- MODULE Pc_C_process ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--fair algorithm A { variable x = 0; fair + process (P \in {1, 2}) variable a = 3; { L: skip; } process (Q = 3) { M: skip; } }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "cd3e3d50")
VARIABLES pc, x, a

vars == << pc, x, a >>

ProcSet == ({1, 2}) \cup {3}

Init == (* Global variables *)
        /\ x = 0
        (* Process P *)
        /\ a = [self \in {1, 2} |-> 3]
        /\ pc = [self \in ProcSet |-> CASE self \in {1, 2} -> "L"
                                        [] self = 3 -> "M"]

L(self) == /\ pc[self] = "L"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, a >>

P(self) == L(self)

M == /\ pc[3] = "M"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![3] = "Done"]
     /\ UNCHANGED << x, a >>

Q == M

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q
           \/ (\E self \in {1, 2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)
        /\ \A self \in {1, 2} : SF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
