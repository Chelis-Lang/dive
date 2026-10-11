---- MODULE AcyclicHeader_C_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p); goto O3; O2:x:=99; O3:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "f54b9c2e" /\ chksum(tla) = "4dbcd604")
VARIABLES pc, x, stack, p, q

vars == << pc, x, stack, p, q >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ p = 0
        (* Procedure Inner *)
        /\ q = 0
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ /\ q' = p
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O3",
                          q         |->  q ] >>
                      \o stack
      /\ pc' = "I1"
      /\ UNCHANGED << x, p >>

O2 == /\ pc = "O2"
      /\ x' = 99
      /\ pc' = "O3"
      /\ UNCHANGED << stack, p, q >>

O3 == /\ pc = "O3"
      /\ pc' = Head(stack).pc
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q >>

Outer == O1 \/ O2 \/ O3

I1 == /\ pc = "I1"
      /\ x' = q
      /\ pc' = "I2"
      /\ UNCHANGED << stack, p, q >>

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, p >>

Inner == I1 \/ I2

L == /\ pc = "L"
     /\ /\ p' = 1
        /\ stack' = << [ procedure |->  "Outer",
                         pc        |->  "E",
                         p         |->  p ] >>
                     \o stack
     /\ pc' = "O1"
     /\ UNCHANGED << x, q >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, p, q >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Outer \/ Inner \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
