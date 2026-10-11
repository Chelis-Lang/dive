---- MODULE Define_C_pc_collision ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 pc == 1
 }
{ L: x := pc; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "1ed746ee" /\ chksum(tla) = "7bee6462")
VARIABLES pc, x, y

(* define statement *)
pc == 1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = pc
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
