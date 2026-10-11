---- MODULE P_nonliteral_domain ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P \in Nat begin L:g:=g+1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1ec733" /\ chksum(tla) = "f18b6239")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == (Nat)

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = g+1
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in Nat: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
