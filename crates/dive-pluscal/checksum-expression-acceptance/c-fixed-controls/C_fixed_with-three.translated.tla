---- MODULE C_fixed_with_three ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(a=1,b=2,c=3){x:=a+b+c;}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "158909ee" /\ chksum(tla) = "a1179e35")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
          LET b == 2 IN
            LET c == 3 IN
              x' = a+b+c
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
