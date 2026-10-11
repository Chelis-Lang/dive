---- MODULE Define_P_state_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == x
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: x := 1; y := F; end process;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e6ed1761" /\ chksum(tla) = "c387d07e")
VARIABLES pc, x, y

(* define statement *)
F == x

VARIABLE z

vars == << pc, x, y, z >>

ProcSet == ({ 1 , 2 })

Init == /\ x = 0
        /\ y = 0
        /\ z = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = 1
           /\ y' = F
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ z' = z

P(self) == L(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 
====
