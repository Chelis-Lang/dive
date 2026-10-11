---- MODULE MultipleOwner_P_duplicate_slots ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(z=0,a=0)  begin G1:x:=a+z; G2:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "73da6b91" /\ chksum(tla) = "f23f54bf")
\* Parameter z of procedure Put at line 3 col 46 changed to z_
\* Parameter a of procedure Put at line 3 col 50 changed to a_
VARIABLES pc, x, stack, z_, a_, z, a

vars == << pc, x, stack, z_, a_, z, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z_ = 0
        /\ a_ = 0
        (* Procedure Get *)
        /\ z = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P1 == /\ pc = "P1"
      /\ x' = a_+z_
      /\ pc' = "P2"
      /\ UNCHANGED << stack, z_, a_, z, a >>

P2 == /\ pc = "P2"
      /\ pc' = Head(stack).pc
      /\ z_' = Head(stack).z_
      /\ a_' = Head(stack).a_
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z, a >>

Put == P1 \/ P2

G1 == /\ pc = "G1"
      /\ x' = a+z
      /\ pc' = "G2"
      /\ UNCHANGED << stack, z_, a_, z, a >>

G2 == /\ pc = "G2"
      /\ pc' = Head(stack).pc
      /\ z' = Head(stack).z
      /\ a' = Head(stack).a
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, z_, a_ >>

Get == G1 \/ G2

L == /\ pc = "L"
     /\ /\ a_' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         z_        |->  z_,
                         a_        |->  a_ ] >>
                     \o stack
        /\ z_' = 1
     /\ pc' = "P1"
     /\ UNCHANGED << x, z, a >>

M == /\ pc = "M"
     /\ /\ a' = 4
        /\ stack' = << [ procedure |->  "Get",
                         pc        |->  "E",
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 3
     /\ pc' = "G1"
     /\ UNCHANGED << x, z_, a_ >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z_, a_, z, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Get \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
