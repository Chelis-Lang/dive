---- MODULE Define_P_parameter_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: x := F(1); end process;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "54fc38e3" /\ chksum(tla) = "a7b61611")
VARIABLES pc, x, y

(* define statement *)
F(a) == a + 1

VARIABLE z

vars == << pc, x, y, z >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        (* Process P *)
        /\ z = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = F(1)
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << y, z >>

P(self) == L(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
