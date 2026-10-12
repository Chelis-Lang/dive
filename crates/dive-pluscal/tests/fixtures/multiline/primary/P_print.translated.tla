---- MODULE P_print ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:print <<x,
  y>>; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4c057228" /\ chksum(tla) = "336eb57a")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ PrintT(                                                <<x,
               y>>)
     /\ pc' = "E"
     /\ UNCHANGED << x, y >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
