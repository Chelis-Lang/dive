---- MODULE Context_P_macro ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
macro M(a) begin x := F(a); end macro;
begin L: M(1); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "92f0e610" /\ chksum(tla) = "d42d767b")
VARIABLES pc, x, y

(* define statement *)
F(a) == a + 1


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
