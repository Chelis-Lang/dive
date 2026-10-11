---- MODULE Define_C_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == 1 \* comment
 G == F+1
 }
{ L: x := G; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "f2ad2864" /\ chksum(tla) = "e2559993")
VARIABLES pc, x, y

(* define statement *)
F == 1
G == F+1


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
