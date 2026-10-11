---- MODULE Forms_C_function_state_domain_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in 0..x] == a+1
 }
process (P \in {1,2}) variable z=0; { L:y:=F[0]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "dab6f939" /\ chksum(tla) = "a50cf980")
VARIABLES pc, x, y

(* define statement *)
F[a \in 0..x] == a+1

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
           /\ y' = F[0]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << x, z >>

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
