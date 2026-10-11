---- MODULE P_name_five ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0,abcde=9; process P = 1 begin L:g:=1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d5ae5532" /\ chksum(tla) = "332b4b04")
VARIABLES pc, g, abcde

vars == << pc, g, abcde >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ abcde = 9
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ abcde' = abcde

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, abcde >>

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
