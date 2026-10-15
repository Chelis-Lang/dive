---- MODULE SelfTail ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure Recur(a=0,c=0) variable u=a+c; begin F: call Recur(a-1,a); return; end procedure; begin L: call Recur(2,3); E: skip; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e19a87ee" /\ chksum(tla) = "cc68b3e8")
VARIABLES pc, x, stack, a, c, u

vars == << pc, x, stack, a, c, u >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ a = 0
        /\ c = 0
        /\ u = a+c
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ a' = a-1
        /\ c' = a
     /\ u' = a'+c'
     /\ pc' = "F"
     /\ UNCHANGED << x, stack >>

Recur == F

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ c' = 3
        /\ stack' = << [ procedure |->  "Recur",
                         pc        |->  "E",
                         u         |->  u,
                         a         |->  a,
                         c         |->  c ] >>
                     \o stack
     /\ u' = a'+c'
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, c, u >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
