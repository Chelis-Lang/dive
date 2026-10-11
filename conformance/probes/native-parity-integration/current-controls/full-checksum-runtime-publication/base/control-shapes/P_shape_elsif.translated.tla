---- MODULE P_shape_elsif ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ec16a3e4" /\ chksum(tla) = "126b6396")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ IF x=1
                      THEN /\ x' = 2
                      ELSE /\ TRUE
                           /\ x' = x
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
