---- MODULE Define_P_record ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == [a |-> a, b |-> 1]
 end define;
begin L: x := F(2); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "3ceb0c05" /\ chksum(tla) = "f374f981")
VARIABLES pc, x, y

(* define statement *)
F(a) == [a |-> a, b |-> 1]

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
