---- MODULE Define_C_recursive ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == IF a=0 THEN 0 ELSE F(a-1)
 }
{ L: x := F(2); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "9321f997" /\ chksum(tla) = "959ca906")
VARIABLES pc, x, y

(* define statement *)
F(a) == IF a=0 THEN 0 ELSE F(a-1)


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
