---- MODULE AcyclicHeader_P_local_from_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
variables b=q+1;
begin I1:x:=b; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8264b49d" /\ chksum(tla) = "ef9f6f5c")
VARIABLES pc, x, stack, p, q, b

vars == << pc, x, stack, p, q, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ p = 0
        (* Procedure Inner *)
        /\ q = 0
        /\ b = q+1
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ /\ q' = p+1
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O2",
                          b         |->  b,
                          q         |->  q ] >>
                      \o stack
      /\ b' = q'+1
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
      /\ x' = b
      /\ pc' = "I2"
      /\ UNCHANGED << stack, p, q, b >>

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ b' = Head(stack).b
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
