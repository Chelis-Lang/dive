---- MODULE Define_C_helper ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a+1
 G == F(1)
 }
{ L: x := G; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "686635aa" /\ chksum(tla) = "d737c091")
VARIABLES pc, x, y

(* define statement *)
F(a) == a+1
G == F(1)

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = G
     /\ pc' = "Done"
     /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
