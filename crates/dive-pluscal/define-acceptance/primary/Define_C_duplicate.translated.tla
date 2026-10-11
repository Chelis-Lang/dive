---- MODULE Define_C_duplicate ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == 1
 F == 2
 }
{ L: x := F; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "73d2cc72" /\ chksum(tla) = "aa529070")
VARIABLES pc, x, y

(* define statement *)
F == 1
F == 2


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
