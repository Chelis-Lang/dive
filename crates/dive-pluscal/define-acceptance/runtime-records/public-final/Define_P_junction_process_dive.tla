---- MODULE Define_P_junction_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == /\ x=0
     /\ y=0
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: if F then x := 1; end if; end process;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "5599dde8" /\ chksum(tla) = "5a4f2830")
VARIABLES pc, x, y

(* define statement *)
F == /\ x=0
    /\ y=0

VARIABLE z

vars == << pc, x, y, z >>

ProcSet == ({ 1 , 2 })

Init == /\ x = 0
        /\ y = 0
        /\ z = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ IF F
                 THEN /\ x' = 1
                 ELSE /\ TRUE
                      /\ x' = x
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
