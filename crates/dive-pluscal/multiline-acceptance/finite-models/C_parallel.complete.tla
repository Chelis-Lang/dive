---- MODULE C_parallel ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=y +
  1 || y:=x +
  2; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "acf771d9" /\ chksum(tla) = "d857d0ae")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ /\ x' = y + 1
        /\ y' = x + 2
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
