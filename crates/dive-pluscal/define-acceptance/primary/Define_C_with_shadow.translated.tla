---- MODULE Define_C_with_shadow ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a+1
 }
{ L: with (F = 2) { x := F; }; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "1b8df56c" /\ chksum(tla) = "afef5ea7")
VARIABLES pc, x, y

(* define statement *)
F(a) == a+1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET F == 2 IN
          x' = F
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
