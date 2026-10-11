---- MODULE Forms_C_local_parameter ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 LOCAL F(a)==a+1
 }
{ L:y:=F(x); E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "873db946" /\ chksum(tla) = "bc6ffa7f")
VARIABLES pc, x, y

(* define statement *)
LOCAL F(a)==a+1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F(x)
     /\ pc' = "E"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
