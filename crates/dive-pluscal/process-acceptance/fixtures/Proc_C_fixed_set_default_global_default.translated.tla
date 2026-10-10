---- MODULE Proc_C_fixed_set_default_global_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g; process (P \in{1,2}) variables x=0;  { L: g:=x; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "47a0608b" /\ chksum(tla) = "662f06b5")
CONSTANT defaultInitValue
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = defaultInitValue
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, x >>

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
