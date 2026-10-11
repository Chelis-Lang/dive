---- MODULE Define_C_prime ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == x'
 }
{ L: x := F; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "9b65a5f4" /\ chksum(tla) = "4a7c824d")
VARIABLES pc, x, y

(* define statement *)
F == x'


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F
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
