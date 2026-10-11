---- MODULE AcyclicHeader_P_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:either call Inner(p); or x:=0; end either; O2:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "754d3be5" /\ chksum(tla) = "e06a731c")
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
      /\ \/ /\ /\ q' = p
               /\ stack' = << [ procedure |->  "Inner",
                                pc        |->  "O2",
                                q         |->  q ] >>
                            \o stack
            /\ pc' = "I1"
            /\ x' = x
         \/ /\ x' = 0
            /\ pc' = "O2"
            /\ UNCHANGED <<stack, q>>
      /\ p' = p

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q >>

Outer == O1 \/ O2

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
