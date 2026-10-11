---- MODULE Forms_C_function_empty ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {}]==a
 }
{ L:y:=Cardinality(DOMAIN F); E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e697476d" /\ chksum(tla) = "86b81f90")
VARIABLES pc, x, y

(* define statement *)
F[a \in {}]==a


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = Cardinality(DOMAIN F)
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
