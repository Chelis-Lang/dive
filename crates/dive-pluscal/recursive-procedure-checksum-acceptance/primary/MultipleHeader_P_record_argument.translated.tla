---- MODULE MultipleHeader_P_record_argument ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0)  begin P1:x:=a.k; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:call Put([k |-> 3]); M:call Get(3,4); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "b2101932" /\ chksum(tla) = "8e25408c")
VARIABLES pc, x, stack, a, q, b

vars == << pc, x, stack, a, q, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        (* Procedure Get *)
        /\ q = 0
        /\ b = 0
        /\ stack = << >>
        /\ pc = "L"

P1 == /\ pc = "P1"
      /\ x' = a.k
      /\ pc' = "P2"
      /\ UNCHANGED << stack, a, q, b >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b >>

Put == P1 \/ P2

G1 == /\ pc = "G1"
      /\ x' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, a, q, b >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, a >>

Get == G1 \/ G2

L == /\ pc = "L"
     /\ /\ a' = [k |-> 3]
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         a         |->  a ] >>
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
     /\ UNCHANGED << x, a >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, q, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
