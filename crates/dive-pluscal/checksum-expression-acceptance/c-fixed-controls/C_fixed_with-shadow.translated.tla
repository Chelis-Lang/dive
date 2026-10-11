---- MODULE C_fixed_with_shadow ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(x=1,b=2){y:=x+b;}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f229c520" /\ chksum(tla) = "31e0d441")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET x == 1 IN
          LET b == 2 IN
            y' = x+b
     /\ pc' = "E"
     /\ x' = x

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
