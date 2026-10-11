---- MODULE MultipleHeader_C_local_from_parameter_outside_body ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) variable u=a; { P1:x:=u+z; P2:return; };procedure Get(q=0,b=0) variable v=b; { G1:x:=v+q; G2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "b029bae9" /\ chksum(tla) = "e0b032a4")
VARIABLES pc, x, stack, z, a, u, q, b, v

vars == << pc, x, stack, z, a, u, q, b, v >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        /\ u = a
        (* Procedure Get *)
        /\ q = 0
        /\ b = 0
        /\ v = b
        /\ stack = << >>
        /\ pc = "L"

P1 == /\ pc = "P1"
      /\ x' = u+z
      /\ pc' = "P2"
      /\ UNCHANGED << stack, z, a, u, q, b, v >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ u' = Head(stack).u
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b, v >>

Put == P1 \/ P2

G1 == /\ pc = "G1"
      /\ x' = v+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, z, a, u, q, b, v >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ v' = Head(stack).v
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a, u >>

Get == G1 \/ G2

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         u         |->  u,
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 1
     /\ u' = a'
     /\ pc' = "P1"
     /\ UNCHANGED << x, q, b, v >>

M == /\ pc = "M"
     /\ /\ b' = 4
        /\ q' = 3
        /\ stack' = << [ procedure |->  "Get",
                         pc        |->  "E",
                         v         |->  v,
                         q         |->  q,
                         b         |->  b ] >>
                     \o stack
     /\ v' = b'
     /\ pc' = "G1"
     /\ UNCHANGED << x, z, a, u >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a, u, q, b, v >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
