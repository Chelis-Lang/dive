---- MODULE AcyclicHeader_P_target_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
variables b=0;
begin I1:b:=q+1; I2:x:=b; I3:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "fbb9bcf" /\ chksum(tla) = "e46c6762")
VARIABLES pc, x, stack, p, q, b

vars == << pc, x, stack, p, q, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ p = 0
        (* Procedure Inner *)
        /\ q = 0
        /\ b = 0
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ /\ q' = p+1
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O2",
                          b         |->  b,
                          q         |->  q ] >>
                      \o stack
      /\ b' = 0
      /\ pc' = "I1"
      /\ UNCHANGED << x, p >>

O2 == /\ pc = "O2"
      /\ x' = p
      /\ pc' = "O3"
      /\ UNCHANGED << stack, p, q, b >>

O3 == /\ pc = "O3"
      /\ pc' = Head(stack).pc
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b >>

Outer == O1 \/ O2 \/ O3

I1 == /\ pc = "I1"
      /\ b' = q+1
      /\ pc' = "I2"
      /\ UNCHANGED << x, stack, p, q >>

I2 == /\ pc = "I2"
      /\ x' = b
      /\ pc' = "I3"
      /\ UNCHANGED << stack, p, q, b >>

I3 == /\ pc = "I3"
      /\ pc' = Head(stack).pc
      /\ b' = Head(stack).b
      /\ q' = Head(stack).q
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, p >>

Inner == I1 \/ I2 \/ I3

L == /\ pc = "L"
     /\ /\ p' = 1
        /\ stack' = << [ procedure |->  "Outer",
                         pc        |->  "E",
                         p         |->  p ] >>
                     \o stack
     /\ pc' = "O1"
     /\ UNCHANGED << x, q, b >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, p, q, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Outer \/ Inner \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
