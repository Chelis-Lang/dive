---- MODULE C_fair_string_constant ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = "p"){L:g:=g+1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "9f0b385f")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {"p"}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc["p"] = "L"
     /\ g' = g+1
     /\ pc' = [pc EXCEPT !["p"] = "E"]

E == /\ pc["p"] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["p"] = "Done"]
     /\ g' = g

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
