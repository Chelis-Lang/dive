---- MODULE C_duplicate_label_label_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P=1){L:g:=1; E:skip;} process(Q=2){L:g:=2; F:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "735b282f" /\ chksum(tla) = "304139f")
\* Label L of process P at line 3 col 49 changed to L_
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L_"
                                        [] self = 2 -> "L"]

L_ == /\ pc[1] = "L_"
      /\ g' = 1
      /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L_ \/ E

L == /\ pc[2] = "L"
     /\ g' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]

F == /\ pc[2] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ g' = g

Q == L \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
