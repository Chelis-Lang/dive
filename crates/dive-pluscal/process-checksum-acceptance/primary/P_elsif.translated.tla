---- MODULE P_elsif ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:if g=0 then g:=1; elsif g=1 then g:=2; else g:=3; end if; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1f8f0871" /\ chksum(tla) = "18855d91")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ IF g=0
           THEN /\ g' = 1
           ELSE /\ IF g=1
                      THEN /\ g' = 2
                      ELSE /\ g' = 3
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
