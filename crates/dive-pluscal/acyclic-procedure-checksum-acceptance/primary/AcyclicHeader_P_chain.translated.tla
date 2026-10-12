---- MODULE AcyclicHeader_P_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
begin I1:call Leaf(q+1); I2:return; end procedure;
procedure Leaf(r=0)
begin J1:x:=r; J2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "37b455c8" /\ chksum(tla) = "4ca1ab40")
VARIABLES pc, x, stack, p, q, r

vars == << pc, x, stack, p, q, r >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ p = 0
        (* Procedure Inner *)
        /\ q = 0
        (* Procedure Leaf *)
        /\ r = 0
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ /\ q' = p+1
         /\ stack' = << [ procedure |->  "Inner",
                          pc        |->  "O2",
                          q         |->  q ] >>
                      \o stack
      /\ pc' = "I1"
      /\ UNCHANGED << x, p, r >>

O2 == /\ pc = "O2"
      /\ x' = p
      /\ pc' = "O3"
      /\ UNCHANGED << stack, p, q, r >>

O3 == /\ pc = "O3"
      /\ pc' = Head(stack).pc
      /\ p' = Head(stack).p
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, r >>

Outer == O1 \/ O2 \/ O3

I1 == /\ pc = "I1"
      /\ /\ r' = q+1
         /\ stack' = << [ procedure |->  "Leaf",
                          pc        |->  "I2",
                          r         |->  r ] >>
                      \o stack
      /\ pc' = "J1"
      /\ UNCHANGED << x, p, q >>

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, p, r >>

Inner == I1 \/ I2

J1 == /\ pc = "J1"
      /\ x' = r
      /\ pc' = "J2"
      /\ UNCHANGED << stack, p, q, r >>

J2 == /\ pc = "J2"
      /\ pc' = Head(stack).pc
      /\ r' = Head(stack).r
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, p, q >>

Leaf == J1 \/ J2

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

Next == Outer \/ Inner \/ Leaf \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
