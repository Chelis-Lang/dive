---- MODULE Forms_C_function_parameter_shadow ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[x \in {1,2}]==x+1
 }
{ L:y:=F[1]; E:skip; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "35ef6c35" /\ chksum(tla) = "3738553f")
VARIABLES pc, x, y

(* define statement *)
F[x \in {1,2}]==x+1


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = F[1]
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
