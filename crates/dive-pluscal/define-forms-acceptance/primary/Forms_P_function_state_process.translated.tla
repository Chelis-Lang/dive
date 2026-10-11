---- MODULE Forms_P_function_state_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}] == x+a
 end define;
process P \in {1,2} variable z=0; begin L:x:=1; y:=F[1]; E:skip; end process;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "6b57b5b2" /\ chksum(tla) = "dc9cf709")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}] == x+a

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
           /\ x' = 1
           /\ y' = F[1]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ z' = z

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y, z >>

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
