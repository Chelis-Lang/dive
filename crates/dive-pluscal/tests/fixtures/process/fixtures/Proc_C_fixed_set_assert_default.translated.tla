---- MODULE Proc_C_fixed_set_assert_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: x:=1; assert x=0; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "28aa620a" /\ chksum(tla) = "2ee95449")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ Assert(x'[self]=0, 
                     "Failure of assertion at line 3, column 83.")
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ g' = g

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, x >>

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
