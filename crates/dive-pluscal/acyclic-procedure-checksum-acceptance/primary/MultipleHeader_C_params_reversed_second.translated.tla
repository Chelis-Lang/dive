---- MODULE MultipleHeader_C_params_reversed_second ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(z=0,a=0) { P1:x:=a+z; P2:return; };procedure Get(b=0,q=0) { G1:x:=b+q; G2:return; }; { L:call Put(1,2); M:call Get(3,4); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "92e53d0b" /\ chksum(tla) = "2989b894")
VARIABLES pc, x, stack, z, a, b, q

vars == << pc, x, stack, z, a, b, q >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        (* Procedure Get *)
        /\ b = 0
        /\ q = 0
        /\ stack = << >>
        /\ pc = "L"

P1 == /\ pc = "P1"
      /\ x' = a+z
      /\ pc' = "P2"
      /\ UNCHANGED << stack, z, a, b, q >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, b, q >>

Put == P1 \/ P2

G1 == /\ pc = "G1"
      /\ x' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, z, a, b, q >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ b' = Head(stack).b
      /\ q' = Head(stack).q
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a >>

Get == G1 \/ G2

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 1
     /\ pc' = "P1"
     /\ UNCHANGED << x, b, q >>

M == /\ pc = "M"
     /\ /\ b' = 3
        /\ q' = 4
        /\ stack' = << [ procedure |->  "Get",
                         pc        |->  "E",
                         b         |->  b,
                         q         |->  q ] >>
                     \o stack
     /\ pc' = "G1"
     /\ UNCHANGED << x, z, a >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a, b, q >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
