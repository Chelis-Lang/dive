---- MODULE C_branch_long ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0,abcdef=9; process(P = 1){L:if(g=0){abcdef:=1;}else{g:=1;}; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "cf876b2c" /\ chksum(tla) = "f93c6dd1")
VARIABLES pc, g, abcdef

vars == << pc, g, abcdef >>

ProcSet == {1}

Init == /\ g = 0
        /\ abcdef = 9
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ IF g = 0
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

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 
====
