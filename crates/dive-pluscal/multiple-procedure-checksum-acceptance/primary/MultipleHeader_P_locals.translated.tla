---- MODULE MultipleHeader_P_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) variable u=0; begin P1:u:=a+z; P2:x:=u; P3:return; end procedure;procedure Get(q=0,b=0) variable v=0; begin G1:v:=b+q; G2:x:=v; G3:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "37b3820" /\ chksum(tla) = "79479742")
VARIABLES pc, x, stack, z, a, u, q, b, v

vars == << pc, x, stack, z, a, u, q, b, v >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        /\ u = 0
        (* Procedure Get *)
        /\ q = 0
        /\ b = 0
        /\ v = 0
        /\ stack = << >>
        /\ pc = "L"

P1 == /\ pc = "P1"
      /\ u' = a+z
      /\ pc' = "P2"
      /\ UNCHANGED << x, stack, z, a, q, b, v >>

P2 == /\ pc = "P2"
      /\ x' = u
      /\ pc' = "P3"
      /\ UNCHANGED << stack, z, a, u, q, b, v >>

P3 == /\ pc = "P3"
      /\ pc' = Head(stack).pc
      /\ u' = Head(stack).u
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b, v >>

Put == P1 \/ P2 \/ P3

G1 == /\ pc = "G1"
      /\ v' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << x, stack, z, a, u, q, b >>

G2 == /\ pc = "G2"
      /\ x' = v
      /\ pc' = "G3"
      /\ UNCHANGED << stack, z, a, u, q, b, v >>

G3 == /\ pc = "G3"
      /\ pc' = Head(stack).pc
      /\ v' = Head(stack).v
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a, u >>

Get == G1 \/ G2 \/ G3

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         u         |->  u,
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 1
     /\ u' = 0
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
     /\ v' = 0
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
