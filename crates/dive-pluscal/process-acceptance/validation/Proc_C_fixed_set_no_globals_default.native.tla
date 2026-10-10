---- MODULE Proc_C_fixed_set_no_globals_default ----
EXTENDS Naturals, TLC
(* --algorithm A { process (P \in{1,2}) variables x=0;  { L: x:=1; E: skip; } } *)

VARIABLES pc, x

vars == << pc, x >>

ProcSet == ({ 1 , 2 })

Init == /\ x = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ x' = x

P(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
