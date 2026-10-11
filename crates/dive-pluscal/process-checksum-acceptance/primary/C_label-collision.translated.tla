---- MODULE C_label_collision ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:g:=g+1; E:skip;} process(Q = 2){L:g:=1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "d7ed22d3" /\ chksum(tla) = "826c3673")
\* Label L of process P at line 3 col 51 changed to L_
\* Label E of process P at line 3 col 61 changed to E_
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L_"
                                        [] self = 2 -> "L"]

L_ == /\ pc[1] = "L_"
      /\ g' = g+1
      /\ pc' = [pc EXCEPT ![1] = "E_"]

E_ == /\ pc[1] = "E_"
      /\ TRUE
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ g' = g

P == L_ \/ E_

L == /\ pc[2] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![2] = "E"]

E == /\ pc[2] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ g' = g

Q == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
