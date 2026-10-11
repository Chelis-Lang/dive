---- MODULE P_fair_branch_long ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0,abcdef=9; process P = 1 begin L:if g=0 then abcdef:=1; else g:=1; end if; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "2bc82e5e")
VARIABLES pc, g, abcdef

vars == << pc, g, abcdef >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ abcdef = 9
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ IF g=0
           THEN /\ abcdef' = 1
                /\ g' = g
           ELSE /\ g' = 1
                /\ UNCHANGED abcdef
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, abcdef >>

P == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
