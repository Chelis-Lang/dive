---- MODULE AcyclicHeader_C_caller_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
variables a=0;
{ O0:a:=p+1; O1:call Inner(a); O2:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "cc1e6c9f" /\ chksum(tla) = "ba03389c")
VARIABLES pc, x, stack, p, a, q

vars == << pc, x, stack, p, a, q >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ p = 0
        /\ a = 0
        (* Procedure Inner *)
        /\ q = 0
        /\ stack = << >>
        /\ pc = "L"

O0 == /\ pc = "O0"
      /\ a' = p+1
      /\ pc' = "O1"
      /\ UNCHANGED << x, stack, p, q >>

O1 == /\ pc = "O1"
      /\ /\ q' = a
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O2",
                          q         |->  q ] >>
                      \o stack
      /\ pc' = "I1"
      /\ UNCHANGED << x, p, a >>

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ a' = Head(stack).a
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q >>

Outer == O0 \/ O1 \/ O2

I1 == /\ pc = "I1"
      /\ x' = q
      /\ pc' = "I2"
      /\ UNCHANGED << stack, p, a, q >>

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, p, a >>

Inner == I1 \/ I2

L == /\ pc = "L"
     /\ /\ p' = 1
        /\ stack' = << [ procedure |->  "Outer",
                         pc        |->  "E",
                         a         |->  a,
                         p         |->  p ] >>
                     \o stack
     /\ a' = 0
     /\ pc' = "O0"
     /\ UNCHANGED << x, q >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, p, a, q >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Outer \/ Inner \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
