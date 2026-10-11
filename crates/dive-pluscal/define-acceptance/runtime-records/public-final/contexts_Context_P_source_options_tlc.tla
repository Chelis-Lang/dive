---- MODULE Context_P_source_options ----
EXTENDS Naturals, Integers, Sequences, TLC
(* PlusCal options (-wf) *)
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
begin L: x := F(1); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "92f0e610" /\ chksum(tla) = "4f056498")
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
