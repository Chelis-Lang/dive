---- MODULE Define_C_sequence ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == <<a,a>>
 }
{ L: x := F(2); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "d9a1e2c7" /\ chksum(tla) = "1d0e1868")
VARIABLES pc, x, y

(* define statement *)
F(a) == <<a,a>>

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F ( 2 )
     /\ pc' = "Done"
     /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
