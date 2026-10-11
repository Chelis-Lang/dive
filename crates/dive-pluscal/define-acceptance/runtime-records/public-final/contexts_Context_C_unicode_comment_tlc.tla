---- MODULE Context_C_unicode_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 (* outside λ *) F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "92f0e610" /\ chksum(tla) = "a427785c")
VARIABLES pc, x, y

(* define statement *)
F(a) == a + 1

vars == << pc, x, y >>

Init == /\ x = 0
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
