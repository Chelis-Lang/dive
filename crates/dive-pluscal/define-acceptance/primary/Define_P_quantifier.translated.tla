---- MODULE Define_P_quantifier ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == \A i \in 1..a : i>0
 end define;
begin L: x := F(2); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "d7d87ba" /\ chksum(tla) = "bf730131")
VARIABLES pc, x, y

(* define statement *)
F(a) == \A i \in 1..a : i>0


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
