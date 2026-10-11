---- MODULE Define_C_nested_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == (* note (* inner *) *) 1
 }
{ L: x := F; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "4f4bc0bb" /\ chksum(tla) = "ef3950d4")
VARIABLES pc, x, y

(* define statement *)
F ==                        1


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
