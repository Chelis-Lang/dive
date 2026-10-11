---- MODULE C_fixed_with_two ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(a=1,b=2){x:=a+b;}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "ca3672f2" /\ chksum(tla) = "71851140")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
          LET b == 2 IN
            x' = a+b
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
