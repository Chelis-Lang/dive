---- MODULE WhileTail_C_Test ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P:x:=z+a; R:return; }; { L:while (x=0) { skip; }; if (TRUE) { call Put(1,2); goto E; } else { skip; }; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "c761b32b" /\ chksum(tla) = "18eb467b")
VARIABLES pc, x, stack, z, a

vars == << pc, x, stack, z, a >>

Init == /\ x = 0
        /\ z = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = z + a
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
     /\ IF x = 0
           THEN /\ TRUE
                /\ pc' = "L"
                /\ UNCHANGED << stack, z, a >>
           ELSE /\ IF TRUE
                      THEN /\ /\ a' = 2
                              /\ stack' = << [ procedure |-> "Put", pc |-> "E", z |-> z, a |-> a ] >> \o stack
                              /\ z' = 1
                           /\ pc' = "P"
                      ELSE /\ TRUE
                           /\ pc' = "E"
                           /\ UNCHANGED << stack, z, a >>
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
