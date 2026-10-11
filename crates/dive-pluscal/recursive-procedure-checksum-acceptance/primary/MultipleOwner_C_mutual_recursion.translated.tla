---- MODULE MultipleOwner_C_mutual_recursion ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:call Get(3,4); P3:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:call Put(1,2); G3:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "14d9a786" /\ chksum(tla) = "a4d5d1df")
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

P1 == /\ pc = "P1"
      /\ x' = a+z
      /\ pc' = "P2"
      /\ UNCHANGED << stack, z, a, q, b >>

P2 == /\ pc = "P2"
      /\ /\ b' = 4
         /\ q' = 3
         /\ stack' = << [ procedure |->  "Get",
                          pc        |->  "P3",
                          q         |->  q,
                          b         |->  b ] >>
                      \o stack
      /\ pc' = "G1"
      /\ UNCHANGED << x, z, a >>

P3 == /\ pc = "P3"
      /\ pc' = Head(stack).pc
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b >>

Put == P1 \/ P2 \/ P3

G1 == /\ pc = "G1"
      /\ x' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, z, a, q, b >>

G2 == /\ pc = "G2"
      /\ /\ a' = 2
         /\ stack' = << [ procedure |->  "Put",
                          pc        |->  "G3",
                          z         |->  z,
                          a         |->  a ] >>
                      \o stack
         /\ z' = 1
      /\ pc' = "P1"
      /\ UNCHANGED << x, q, b >>

G3 == /\ pc = "G3"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a >>

Get == G1 \/ G2 \/ G3

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 1
     /\ pc' = "P1"
     /\ UNCHANGED << x, q, b >>

M == /\ pc = "M"
     /\ /\ b' = 4
        /\ q' = 3
        /\ stack' = << [ procedure |->  "Get",
                         pc        |->  "E",
                         q         |->  q,
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "G1"
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
