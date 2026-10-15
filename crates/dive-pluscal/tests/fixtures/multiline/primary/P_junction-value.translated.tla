---- MODULE P_junction_value ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:=IF /\ TRUE
  /\ y=0 THEN 1 ELSE 2; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "43e5a863" /\ chksum(tla) = "45e98bbf")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (                                             IF /\ TRUE
              /\ y=0 THEN 1 ELSE 2)
     /\ pc' = "E"
     /\ y' = y

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
