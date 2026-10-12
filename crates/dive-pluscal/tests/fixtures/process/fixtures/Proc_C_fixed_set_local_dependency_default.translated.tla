---- MODULE Proc_C_fixed_set_local_dependency_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0,y=x+1;  { L: g:=y; E: x:=g; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "cd683e73" /\ chksum(tla) = "1c790ef3")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ y = [self \in {1,2} |-> x[self]+1]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = y[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << x, y >>

E(self) == /\ pc[self] = "E"
           /\ x' = [x EXCEPT ![self] = g]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, y >>

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
