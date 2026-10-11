---- MODULE Context_P_local ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 LOCAL F(a) == a+1
 end define;
begin L: x := F(1); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "3447b4" /\ chksum(tla) = "c5b7c01a")
VARIABLES pc, x, y

(* define statement *)
LOCAL F(a) == a+1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F(1)
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
