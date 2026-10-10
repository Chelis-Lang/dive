---- MODULE Pc_C_elsif ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { variable x = 0, y; {
 if (x = 0) skip; else if (x = 1) skip; else skip;
} }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "7503e269" /\ chksum(tla) = "7afd17c9")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x = 0
               THEN /\ TRUE
               ELSE /\ IF x = 1
                          THEN /\ TRUE
                          ELSE /\ TRUE
         /\ pc' = "Done"
         /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
