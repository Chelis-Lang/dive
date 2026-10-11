---- MODULE MultipleOwner_C_duplicate_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };procedure Get(q=0,b=0) { P1:x:=b+q; P2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "ba3ece39" /\ chksum(tla) = "9bc2b876")
\* Label P1 of procedure Put at line 3 col 62 changed to P1_
\* Label P2 of procedure Put at line 3 col 73 changed to P2_
VARIABLES pc, x, stack, z, a, q, b

vars == << pc, x, stack, z, a, q, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        (* Procedure Get *)
        /\ q = 0
        /\ b = 0
        /\ stack = << >>
        /\ pc = "L"

P1_ == /\ pc = "P1_"
       /\ x' = a+z
       /\ pc' = "P2_"
       /\ UNCHANGED << stack, z, a, q, b >>

P2_ == /\ pc = "P2_"
       /\ pc' = Head(stack).pc
       /\ z' = Head(stack).z
       /\ a' = Head(stack).a
       /\ stack' = Tail(stack)
       /\ UNCHANGED << x, q, b >>

Put == P1_ \/ P2_

P1 == /\ pc = "P1"
      /\ x' = b+q
      /\ pc' = "P2"
      /\ UNCHANGED << stack, z, a, q, b >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a >>

Get == P1 \/ P2

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 1
     /\ pc' = "P1_"
     /\ UNCHANGED << x, q, b >>

M == /\ pc = "M"
     /\ /\ b' = 4
        /\ q' = 3
        /\ stack' = << [ procedure |->  "Get",
                         pc        |->  "E",
                         q         |->  q,
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "P1"
     /\ UNCHANGED << x, z, a >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a, q, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
