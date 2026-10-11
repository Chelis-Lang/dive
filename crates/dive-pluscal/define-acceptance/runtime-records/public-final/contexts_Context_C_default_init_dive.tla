---- MODULE Context_C_default_init ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x, y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e20755ea" /\ chksum(tla) = "a41adbd1")
CONSTANT defaultInitValue

VARIABLES pc, x, y

(* define statement *)
F(a) == a + 1

vars == << pc, x, y >>

Init == /\ x = defaultInitValue
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F ( 1 )
     /\ pc' = "Done"
     /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
