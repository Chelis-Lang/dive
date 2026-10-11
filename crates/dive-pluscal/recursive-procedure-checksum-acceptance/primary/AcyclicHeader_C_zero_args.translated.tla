---- MODULE AcyclicHeader_C_zero_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer()
{ O1:call Inner(); O2:return; };
procedure Inner()
{ I1:x:=x+1; I2:return; };
{ L:call Outer(); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "ab71b36d" /\ chksum(tla) = "a55ae928")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ stack' = << [ procedure |->  "Inner",
                       pc        |->  "O2" ] >>
                   \o stack
      /\ pc' = "I1"
      /\ x' = x

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ stack' = Tail(stack)
      /\ x' = x

Outer == O1 \/ O2

I1 == /\ pc = "I1"
      /\ x' = x+1
      /\ pc' = "I2"
      /\ stack' = stack

I2 == /\ pc = "I2"
      /\ pc' = Head(stack).pc
      /\ stack' = Tail(stack)
      /\ x' = x

Inner == I1 \/ I2

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "Outer",
                      pc        |->  "E" ] >>
                  \o stack
     /\ pc' = "O1"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Outer \/ Inner \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
