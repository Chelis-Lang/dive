---- MODULE Pc_P_quantifier ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A variable x = 0, y; begin
 assert \A a, b \in {1, 2} : a = b;
end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "acd23445" /\ chksum(tla) = "71cdfe0d")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = defaultInitValue
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ Assert(\A a, b \in {1, 2} : a = b, 
                   "Failure of assertion at line 5, column 2.")
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
