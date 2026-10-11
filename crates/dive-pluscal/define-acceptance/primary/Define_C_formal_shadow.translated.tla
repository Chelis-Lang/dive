---- MODULE Define_C_formal_shadow ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(x) == x+1
 }
{ L: x := F(2); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "5ce620c0" /\ chksum(tla) = "74a95f8d")
VARIABLES pc, x, y

(* define statement *)
F(x) == x+1


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
