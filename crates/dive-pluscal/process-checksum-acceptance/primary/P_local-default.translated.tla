---- MODULE P_local_default ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 variables z; begin L:z:=1; E:g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "72d38a38" /\ chksum(tla) = "5a880c36")
CONSTANT defaultInitValue
VARIABLES pc, g, z

vars == << pc, g, z >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z = defaultInitValue
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ z' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ g' = z
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ z' = z

P == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
