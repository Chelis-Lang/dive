---- MODULE MultipleHeader_P_reverse_declarations ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure;procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8197b276" /\ chksum(tla) = "523f6536")
VARIABLES pc, x, stack, q, b, z, a

vars == << pc, x, stack, q, b, z, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Get *)
        /\ q = 0
        /\ b = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

G1 == /\ pc = "G1"
      /\ x' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, q, b, z, a >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a >>

Get == G1 \/ G2

P1 == /\ pc = "P1"
      /\ x' = a+z
      /\ pc' = "P2"
      /\ UNCHANGED << stack, q, b, z, a >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b >>

Put == P1 \/ P2

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
     /\ UNCHANGED << x, stack, q, b, z, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Get \/ Put \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
