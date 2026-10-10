---- MODULE Recursive_C_self_twice ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); }else{ skip; }; R:if(a>0){ call First(a-1); }else{ skip; }; Z:x:=x+1; return;
};
{
L:call First(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "de5c06fb" /\ chksum(tla) = "49951f6a")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ /\ a' = a-1
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  "R",
                                    a         |->  a ] >>
                                \o stack
                /\ pc' = "F"
           ELSE /\ TRUE
                /\ pc' = "R"
                /\ UNCHANGED << stack, a >>
     /\ x' = x

R == /\ pc = "R"
     /\ IF a>0
           THEN /\ /\ a' = a-1
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  "Z",
                                    a         |->  a ] >>
                                \o stack
                /\ pc' = "F"
           ELSE /\ TRUE
                /\ pc' = "Z"
                /\ UNCHANGED << stack, a >>
     /\ x' = x

Z == /\ pc = "Z"
     /\ x' = x+1
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

First == F \/ R \/ Z

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
