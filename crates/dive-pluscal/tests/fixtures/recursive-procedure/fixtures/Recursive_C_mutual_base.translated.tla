---- MODULE Recursive_C_mutual_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call Second(a-1); }else{ x:=0; }; R:x:=x+a; return;
};
procedure Second(b=0)

{
S:if(b>0){ call First(b-1); }else{ x:=1; }; T:x:=x+b; return;
};
{
L:call First(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "c47b215c" /\ chksum(tla) = "f76dd33b")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ /\ b' = a-1
                   /\ stack' = << [ procedure |->  "Second",
                                    pc        |->  "R",
                                    b         |->  b ] >>
                                \o stack
                /\ pc' = "S"
                /\ x' = x
           ELSE /\ x' = 0
                /\ pc' = "R"
                /\ UNCHANGED << stack, b >>
     /\ a' = a

R == /\ pc = "R"
     /\ x' = x+a
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ b' = b

First == F \/ R

S == /\ pc = "S"
     /\ IF b>0
           THEN /\ /\ a' = b-1
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  "T",
                                    a         |->  a ] >>
                                \o stack
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = 1
                /\ pc' = "T"
                /\ UNCHANGED << stack, a >>
     /\ b' = b

T == /\ pc = "T"
     /\ x' = x+b
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ a' = a

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
