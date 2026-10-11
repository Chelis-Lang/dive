---- MODULE Context_C_no_pc ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 }
{ L: while (TRUE) { x := F(1); }; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "98e1edb4" /\ chksum(tla) = "3c29dc3a")
VARIABLES x, y

(* define statement *)
F(a) == a + 1

vars == << x, y >>

Init == /\ x = 0
        /\ y = 0

Next == /\ x' = F ( 1 )
        /\ y' = y

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
