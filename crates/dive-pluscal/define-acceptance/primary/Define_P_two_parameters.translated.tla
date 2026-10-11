---- MODULE Define_P_two_parameters ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a,b) == a+b
 end define;
begin L: x := F(1,2); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "1ae2fe22" /\ chksum(tla) = "ddae6c4f")
VARIABLES pc, x, y

(* define statement *)
F(a,b) == a+b


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F(1,2)
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
