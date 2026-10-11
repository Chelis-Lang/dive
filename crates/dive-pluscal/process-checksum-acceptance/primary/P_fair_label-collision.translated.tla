---- MODULE P_fair_label_collision ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin L:g:=g+1; E:skip; end process; process Q = 2 begin L:g:=1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "10a6614")
\* Label L of process P at line 3 col 60 changed to L_
\* Label E of process P at line 3 col 70 changed to E_
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
