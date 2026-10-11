---- MODULE P_with ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:with a\in {1,2} do g:=a; end with; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ad1bc77f" /\ chksum(tla) = "63c12beb")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ \E a \in {1,2}:
          g' = a
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

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
