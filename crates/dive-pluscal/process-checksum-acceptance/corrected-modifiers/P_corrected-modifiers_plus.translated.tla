---- MODULE P_corrected_modifiers_plus ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; fair process P = 1 begin L:+g:=1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a4c8d318" /\ chksum(tla) = "f3e8e4f0")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(P) /\ SF_vars(L)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
