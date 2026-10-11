---- MODULE P_fair_loop_label_root ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P \in {1,2} begin while g<2 do g:=g+1; end while; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "a5ce121f")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Auto1"]

Auto1(self) == /\ pc[self] = "Auto1"
               /\ IF g<2
                     THEN /\ g' = g+1
                          /\ pc' = [pc EXCEPT ![self] = "Auto1"]
                     ELSE /\ pc' = [pc EXCEPT ![self] = "Done"]
                          /\ g' = g

P(self) == Auto1(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
