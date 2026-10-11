---- MODULE Forms_C_local_parameter_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F(a)==a+1
 }
process (P \in {1,2}) variable z=0; { L:y:=F(x); E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "c8a24c39" /\ chksum(tla) = "d51739f9")
VARIABLES pc, x, y

(* define statement *)
LOCAL F(a)==a+1

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
           /\ y' = F(x)
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
