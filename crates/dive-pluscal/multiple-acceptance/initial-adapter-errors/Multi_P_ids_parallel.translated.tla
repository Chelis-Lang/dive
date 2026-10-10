---- MODULE Multi_P_ids_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: x:=1 || g:=2; E: skip; end process;  process Q =2 variable y=0; begin M: y:=2 || g:=1; F: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8fc7aee7" /\ chksum(tla) = "602ec8bd")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ /\ g' = 2
        /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ y' = y

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, x, y >>

P == L \/ E

M == /\ pc[2] = "M"
     /\ /\ g' = 1
        /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

F == /\ pc[2] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << g, x, y >>

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
