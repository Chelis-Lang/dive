---- MODULE MultipleHeader_C_zero_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put() { P1:x:=3; P2:return; };procedure Get(q=0,b=0) { G1:x:=b+q; G2:return; }; { L:call Put(); M:call Get(3,4); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "d2137595" /\ chksum(tla) = "806a4fc2")
VARIABLES pc, x, stack, q, b

vars == << pc, x, stack, q, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Get *)
        /\ q = 0
        /\ b = 0
        /\ stack = << >>
        /\ pc = "L"

P1 == /\ pc = "P1"
      /\ x' = 3
      /\ pc' = "P2"
      /\ UNCHANGED << stack, q, b >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b >>

Put == P1 \/ P2

G1 == /\ pc = "G1"
      /\ x' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, q, b >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ x' = x

Get == G1 \/ G2

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "Put",
                      pc        |->  "M" ] >>
                  \o stack
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
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, q, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
