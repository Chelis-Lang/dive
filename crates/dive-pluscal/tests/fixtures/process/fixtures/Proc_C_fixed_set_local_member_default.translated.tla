---- MODULE Proc_C_fixed_set_local_member_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x\in{0,1};  { L: g:=x; E: x:=0; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "896b7a9b" /\ chksum(tla) = "54be9127")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x \in [{1,2} -> {0,1}]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ x' = [x EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
