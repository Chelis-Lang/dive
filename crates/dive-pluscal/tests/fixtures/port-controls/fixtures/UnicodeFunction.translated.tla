---- MODULE UnicodeFunction ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; begin L: x:=1; print [k ∈ {1} ↦ k+x]; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "6295c4bf" /\ chksum(tla) = "79c7f3ca")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ PrintT([k ∈ {1} ↦ k+x'])
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
