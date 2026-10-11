---- MODULE Context_C_function_definition ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F[a \in {1,2}] == a+1
 }
{ L: x := F[1]; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "f102eed1" /\ chksum(tla) = "d8ad5074")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}] == a+1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F[1]
     /\ pc' = "Done"
     /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
