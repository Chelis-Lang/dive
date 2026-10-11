---- MODULE P_default_name ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0,defaultInitValue=9; process P = 1 begin L:g:=1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4a066f6c" /\ chksum(tla) = "a74e2a02")
VARIABLES pc, g, defaultInitValue

vars == << pc, g, defaultInitValue >>

ProcSet == {1}

Init == /\ g = 0
        /\ defaultInitValue = 9
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED defaultInitValue

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, defaultInitValue >>

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 

Good == g = 0
====
