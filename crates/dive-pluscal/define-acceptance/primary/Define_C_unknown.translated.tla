---- MODULE Define_C_unknown ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == Unknown
 }
{ L: x := F; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "35578790" /\ chksum(tla) = "892618d0")
VARIABLES pc, x, y

(* define statement *)
F == Unknown


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
