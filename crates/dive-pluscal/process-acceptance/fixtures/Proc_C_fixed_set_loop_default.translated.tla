---- MODULE Proc_C_fixed_set_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: while (x<2) { x:=x+1; }; E: g:=x; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "8590bc92" /\ chksum(tla) = "94cc6f57")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ IF x[self]<2
                 THEN /\ x' = [x EXCEPT ![self] = x[self]+1]
                      /\ pc' = [pc EXCEPT ![self] = "L"]
                 ELSE /\ pc' = [pc EXCEPT ![self] = "E"]
                      /\ x' = x
           /\ g' = g

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ x' = x

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
