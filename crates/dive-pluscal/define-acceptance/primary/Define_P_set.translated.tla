---- MODULE Define_P_set ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == {a,1}
 end define;
begin L: x := F(2); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "719c997a" /\ chksum(tla) = "71356b93")
VARIABLES pc, x, y

(* define statement *)
F(a) == {a,1}


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
