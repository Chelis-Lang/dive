---- MODULE Define_C_constant_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == 1
 }
 process (P \in {1,2})
 variables z=0;
 { L: x := F; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "a7fb8be9" /\ chksum(tla) = "7d066d52")
VARIABLES pc, x, y

(* define statement *)
F == 1

VARIABLE z

vars == << pc, x, y, z >>

ProcSet == ({ 1 , 2 })

Init == /\ x = 0
        /\ y = 0
        /\ z = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = F
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << y, z >>

P(self) == L(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 
====
