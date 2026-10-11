---- MODULE P_shape_while_mixed ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: while x<2 do x:=x+1; B: skip; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7f1c2782" /\ chksum(tla) = "65cd468")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ x' = x+1
                /\ pc' = "B"
           ELSE /\ pc' = "Done"
                /\ x' = x

B == /\ pc = "B"
     /\ TRUE
     /\ pc' = "L"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ B
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
