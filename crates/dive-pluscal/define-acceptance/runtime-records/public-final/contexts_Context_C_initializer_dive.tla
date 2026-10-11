---- MODULE Context_C_initializer ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=F(1), y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "706de5ef" /\ chksum(tla) = "8fad5b99")
VARIABLES pc, x, y

(* define statement *)
F(a) == a + 1

vars == << pc, x, y >>

Init == /\ x = F ( 1 )
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
