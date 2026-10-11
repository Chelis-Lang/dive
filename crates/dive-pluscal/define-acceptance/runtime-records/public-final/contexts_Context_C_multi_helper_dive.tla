---- MODULE Context_C_multi_helper ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 G(a) == F(a) + 1
 }
{ L: x := G(1); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "94db5f88" /\ chksum(tla) = "8480333c")
VARIABLES pc, x, y

(* define statement *)
F(a) == a + 1
G(a) == F(a) + 1

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = G ( 1 )
     /\ pc' = "Done"
     /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
