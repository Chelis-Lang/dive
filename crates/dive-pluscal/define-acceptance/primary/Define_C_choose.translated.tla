---- MODULE Define_C_choose ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == CHOOSE i \in 1..a : TRUE
 }
{ L: x := F(2); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e499b188" /\ chksum(tla) = "f0f7e85a")
VARIABLES pc, x, y

(* define statement *)
F(a) == CHOOSE i \in 1..a : TRUE


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F(2)
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
