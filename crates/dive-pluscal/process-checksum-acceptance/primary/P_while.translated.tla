---- MODULE P_while ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:while g<2 do g:=g+1; end while; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "62eb5952" /\ chksum(tla) = "a9daa5a3")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ IF g<2
           THEN /\ g' = g+1
                /\ pc' = [pc EXCEPT ![1] = "L"]
           ELSE /\ pc' = [pc EXCEPT ![1] = "E"]
                /\ g' = g

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
