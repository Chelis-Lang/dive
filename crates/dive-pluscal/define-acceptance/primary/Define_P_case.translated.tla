---- MODULE Define_P_case ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == CASE a=1 -> 2 [] OTHER -> 0
 end define;
begin L: x := F(1); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "66c01a25" /\ chksum(tla) = "3c420ed9")
VARIABLES pc, x, y

(* define statement *)
F(a) == CASE a=1 -> 2 [] OTHER -> 0


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
