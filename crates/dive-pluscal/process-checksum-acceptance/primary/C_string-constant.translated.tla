---- MODULE C_string_constant ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = "p"){L:g:=g+1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "b4ea5ec2" /\ chksum(tla) = "5cf000db")
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

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
