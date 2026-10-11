---- MODULE Define_P_generated_collision ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 Next == TRUE
 end define;
begin L: x := 1; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "1f985348" /\ chksum(tla) = "1c58e826")
VARIABLES pc, x, y

(* define statement *)
Next == TRUE


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
