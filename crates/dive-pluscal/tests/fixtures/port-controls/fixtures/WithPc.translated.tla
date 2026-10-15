---- MODULE WithPc ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; begin L: with pc=1 do skip; end with; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "334296b9" /\ chksum(tla) = "363126df")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET pc == 1 IN
          TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
