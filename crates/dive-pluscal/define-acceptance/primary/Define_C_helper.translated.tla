---- MODULE Define_C_helper ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a+1
 G == F(1)
 }
{ L: x := G; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "686635aa" /\ chksum(tla) = "3cc6c8d3")
VARIABLES pc, x, y

(* define statement *)
F(a) == a+1
G == F(1)


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = G
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
