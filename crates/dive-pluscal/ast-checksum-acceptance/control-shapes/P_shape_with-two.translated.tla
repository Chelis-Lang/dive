---- MODULE P_shape_with_two ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: with a=1,b=2 do x:=a+b; end with; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "e2bc84a9" /\ chksum(tla) = "df64b552")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
          LET b == 2 IN
            x' = a+b
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
