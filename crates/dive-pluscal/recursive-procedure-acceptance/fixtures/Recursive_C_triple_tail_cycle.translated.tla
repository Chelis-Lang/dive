---- MODULE Recursive_C_triple_tail_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call Second(a-1); return; }else{ x:=a; }; R:return;
};
procedure Second(b=0)

{
S:if(b>0){ call Third(b-1); return; }else{ x:=b; }; T:return;
};
procedure Third(c=0)

{
U:if(c>0){ call First(c-1); return; }else{ x:=c; }; V:return;
};
{
L:call First(4); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7b3a4e12" /\ chksum(tla) = "37dff4d4")
VARIABLES pc, x, stack, a, b, c

vars == << pc, x, stack, a, b, c >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 0
        (* Procedure Third *)
        /\ c = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ /\ b' = a-1
                   /\ stack' = << [ procedure |->  "Second",
                                    pc        |->  Head(stack).pc,
                                    b         |->  b ] >>
                                \o Tail(stack)
                /\ pc' = "S"
                /\ x' = x
           ELSE /\ x' = a
                /\ pc' = "R"
                /\ UNCHANGED << stack, b >>
     /\ UNCHANGED << a, c >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, c >>

First == F \/ R

S == /\ pc = "S"
     /\ IF b>0
           THEN /\ /\ c' = b-1
                   /\ stack' = << [ procedure |->  "Third",
                                    pc        |->  Head(stack).pc,
                                    c         |->  c ] >>
                                \o Tail(stack)
                /\ pc' = "U"
                /\ x' = x
           ELSE /\ x' = b
                /\ pc' = "T"
                /\ UNCHANGED << stack, c >>
     /\ UNCHANGED << a, b >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c >>

Second == S \/ T

U == /\ pc = "U"
     /\ IF c>0
           THEN /\ /\ a' = c-1
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  Head(stack).pc,
                                    a         |->  a ] >>
                                \o Tail(stack)
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = c
                /\ pc' = "V"
                /\ UNCHANGED << stack, a >>
     /\ UNCHANGED << b, c >>

V == /\ pc = "V"
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, b >>

Third == U \/ V

L == /\ pc = "L"
     /\ /\ a' = 4
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b, c >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b, c >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ Third \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
