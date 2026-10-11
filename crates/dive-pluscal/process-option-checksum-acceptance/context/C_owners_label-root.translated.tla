---- MODULE C_owners_label_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair+ process(P=1){L:-g:=1; M:+g:=2; E:skip;} fair process(Q=2){N:+g:=2; R:-g:=1; F:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "32d24b78" /\ chksum(tla) = "58c9f2b8")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "N"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "M"]

M == /\ pc[1] = "M"
     /\ g' = 2
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ M \/ E

N == /\ pc[2] = "N"
     /\ g' = 2
     /\ pc' = [pc EXCEPT ![2] = "R"]

R == /\ pc[2] = "R"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![2] = "F"]

F == /\ pc[2] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ g' = g

Q == N \/ R \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ SF_vars((pc[1] # "L") /\ P)
        /\ WF_vars((pc[2] # "R") /\ Q) /\ SF_vars(N)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
