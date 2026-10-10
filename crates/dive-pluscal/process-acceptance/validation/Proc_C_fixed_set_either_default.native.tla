---- MODULE Proc_C_fixed_set_either_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: either { x:=1; } or { g:=2; }; E: skip; } } *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ \/ /\ x' = [x EXCEPT ![self] = 1]
                 /\ g' = g
              \/ /\ g' = 2
                 /\ x' = x
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, x >>

P(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
