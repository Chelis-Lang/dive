---- MODULE Define_C_variable_collision ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 x == 1
 }
{ L: x := 2; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "6cf10202" /\ chksum(tla) = "cc91fba0")
VARIABLES pc, x, y

(* define statement *)
x == 1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 2
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
