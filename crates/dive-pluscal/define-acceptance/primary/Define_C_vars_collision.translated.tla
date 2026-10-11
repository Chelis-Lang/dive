---- MODULE Define_C_vars_collision ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 vars == <<x>>
 }
{ L: x := 1; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "2f334c10" /\ chksum(tla) = "fa79010a")
VARIABLES pc, x, y

(* define statement *)
vars == <<x>>


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
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
