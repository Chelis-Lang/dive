---- MODULE WhileTail_C_TRUE ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P:x:=z+a; R:return; }; { L:while (TRUE) { skip; }; if (TRUE) { call Put(1,2); goto E; } else { skip; }; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "59824eac" /\ chksum(tla) = "ae79eb8a")
VARIABLES pc, x, stack, z, a

vars == << pc, x, stack, z, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = z+a
     /\ pc' = "R"
     /\ UNCHANGED << stack, z, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ z' = Head(stack).z
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ TRUE
     /\ pc' = "L"
     /\ UNCHANGED << x, stack, z, a >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
