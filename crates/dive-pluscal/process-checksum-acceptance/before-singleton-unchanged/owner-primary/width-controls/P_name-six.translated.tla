---- MODULE P_name_six ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0,abcdef=9; process P = 1 begin L:g:=1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "179a2a28" /\ chksum(tla) = "b6adc62")
VARIABLES pc, g, abcdef

vars == << pc, g, abcdef >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ abcdef = 9
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED abcdef

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, abcdef >>

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
