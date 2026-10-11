---- MODULE DefineGuide_P ----
EXTENDS Integers
(*
--algorithm A
 variable x=0;
 define Step(a) == a+1 end define;
 begin L:x:=Step(x); E:skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "bb328c33" /\ chksum(tla) = "ee5287b5")
VARIABLES pc, x

(* define statement *)
Step(a) == a+1


vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = Step(x)
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
