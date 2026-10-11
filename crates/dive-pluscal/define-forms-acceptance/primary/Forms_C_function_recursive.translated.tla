---- MODULE Forms_C_function_recursive ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}]==IF a=1 THEN 0 ELSE F[a-1]
 }
{ L:y:=F[2]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "ad97e45c" /\ chksum(tla) = "7adab161")
VARIABLES pc, x, y

(* define statement *)
F[a \in {1,2}]==IF a=1 THEN 0 ELSE F[a-1]


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F[2]
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
