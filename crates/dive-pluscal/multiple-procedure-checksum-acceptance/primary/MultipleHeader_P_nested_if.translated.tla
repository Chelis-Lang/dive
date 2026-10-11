---- MODULE MultipleHeader_P_nested_if ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:if x=0 then if x<1 then call Put(1,2); goto E; else call Get(3,4); goto E; end if; else skip; end if; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "6db6f104" /\ chksum(tla) = "84646780")
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
      /\ pc' = Head(stack).pc
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, q, b >>

Put == P1 \/ P2

G1 == /\ pc = "G1"
      /\ x' = b+q
      /\ pc' = "G2"
      /\ UNCHANGED << stack, z, a, q, b >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ b' = Head(stack).b
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a >>

Get == G1 \/ G2

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ IF x<1
                      THEN /\ /\ a' = 2
                              /\ stack' = << [ procedure |->  "Put",
                                               pc        |->  "E",
                                               z         |->  z,
                                               a         |->  a ] >>
                                           \o stack
                              /\ z' = 1
                           /\ pc' = "P1"
                           /\ UNCHANGED << q, b >>
                      ELSE /\ /\ b' = 4
                              /\ q' = 3
                              /\ stack' = << [ procedure |->  "Get",
                                               pc        |->  "E",
                                               q         |->  q,
                                               b         |->  b ] >>
                                           \o stack
                           /\ pc' = "G1"
                           /\ UNCHANGED << z, a >>
           ELSE /\ TRUE
                /\ pc' = "E"
                /\ UNCHANGED << stack, z, a, q, b >>
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a, q, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
