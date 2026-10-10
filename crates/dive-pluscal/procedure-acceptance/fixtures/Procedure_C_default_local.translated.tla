---- MODULE Procedure_C_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable b;
{
P: x:=a; R: return;
};
{
L: call Put(2); E: skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "75591b26" /\ chksum(tla) = "aa805d8f")
CONSTANT defaultInitValue
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ b = defaultInitValue
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = a
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, b >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         b         |->  b,
                         a         |->  a ] >>
                     \o stack
     /\ b' = defaultInitValue
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
