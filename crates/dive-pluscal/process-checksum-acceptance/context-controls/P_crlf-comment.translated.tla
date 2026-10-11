---- MODULE P_crlf_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; fair+ (* π🙂 *) process P = 1 begin L:g:=1; E:skip; end (* π🙂 *) process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ef7524ae" /\ chksum(tla) = "3662afb7")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]

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

Spec == /\ Init /\ [][Next]_vars
        /\ SF_vars(P)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
