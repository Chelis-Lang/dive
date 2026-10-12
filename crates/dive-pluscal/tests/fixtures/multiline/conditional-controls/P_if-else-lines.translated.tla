---- MODULE P_if_else_lines ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:=IF TRUE THEN 1
  ELSE 0; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "90b882e0" /\ chksum(tla) = "2552d2e4")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                              IF TRUE THEN 1
             ELSE 0
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
