---- MODULE Pc_C_labels ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { variable x = 0, y; {
 L: + x := 1; M: - skip;
} }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "925b38c2" /\ chksum(tla) = "808b093e")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "M"
     /\ y' = y

M == /\ pc = "M"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ M
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
