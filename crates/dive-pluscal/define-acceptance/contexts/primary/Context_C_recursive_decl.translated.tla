---- MODULE Context_C_recursive_decl ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 RECURSIVE F(_)
 F(a) == IF a=0 THEN 0 ELSE F(a-1)
 }
{ L: x := F(1); }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "d458aa30" /\ chksum(tla) = "13d0363e")
VARIABLES pc, x, y

(* define statement *)
RECURSIVE F(_)
F(a) == IF a=0 THEN 0 ELSE F(a-1)


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F(1)
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
