---- MODULE AcyclicHeader_C_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
procedure Outer(p=0)
{ O1:call Inner(p+1); O2:x:=p; O3:return; };
{ L:call Outer(1); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "90b81514" /\ chksum(tla) = "1c4b5423")
VARIABLES pc, x, stack, q, p

vars == << pc, x, stack, q, p >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Inner *)
        /\ q = 0
        (* Procedure Outer *)
        /\ p = 0
        /\ stack = << >>
        /\ pc = "L"

I1 == /\ pc = "I1"
      /\ x' = q
      /\ pc' = "I2"
      /\ UNCHANGED << stack, q, p >>

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, p >>

Inner == I1 \/ I2

O1 == /\ pc = "O1"
      /\ /\ q' = p+1
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O2",
                          q         |->  q ] >>
                      \o stack
      /\ pc' = "I1"
      /\ UNCHANGED << x, p >>

O2 == /\ pc = "O2"
      /\ x' = p
      /\ pc' = "O3"
      /\ UNCHANGED << stack, q, p >>

O3 == /\ pc = "O3"
      /\ pc' = Head(stack).pc
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q >>

Outer == O1 \/ O2 \/ O3

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
     /\ UNCHANGED << x, stack, q, p >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Inner \/ Outer \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
