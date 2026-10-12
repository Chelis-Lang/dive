---- MODULE AcyclicHeader_P_simultaneous_arguments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1,p); O2:return; end procedure;
procedure Inner(q=0,r=0)
begin I1:x:=q+r; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "da215ef8" /\ chksum(tla) = "730cf88c")
VARIABLES pc, x, stack, p, q, r

vars == << pc, x, stack, p, q, r >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ p = 0
        (* Procedure Inner *)
        /\ q = 0
        /\ r = 0
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ /\ q' = p+1
         /\ r' = p
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O2",
                          q         |->  q,
                          r         |->  r ] >>
                      \o stack
      /\ pc' = "I1"
      /\ UNCHANGED << x, p >>

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, r >>

Outer == O1 \/ O2

I1 == /\ pc = "I1"
      /\ x' = q+r
      /\ pc' = "I2"
      /\ UNCHANGED << stack, p, q, r >>

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ r' = Head(stack).r
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
     /\ UNCHANGED << x, q, r >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, p, q, r >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Outer \/ Inner \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
