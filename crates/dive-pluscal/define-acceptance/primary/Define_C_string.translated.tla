---- MODULE Define_C_string ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == "ASCII"
 }
{ L: x := F; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "52ddcc2b" /\ chksum(tla) = "765b92b7")
VARIABLES pc, x, y

(* define statement *)
F == "ASCII"


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
