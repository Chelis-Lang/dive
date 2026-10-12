---- MODULE Recursive_P_false_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if FALSE then call First(a); else x:=a; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5fb7a53f" /\ chksum(tla) = "6957e1f0")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF FALSE
           THEN /\ /\ a' = a
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  "R",
                                    a         |->  a ] >>
                                \o stack
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = a
                /\ pc' = "R"
                /\ UNCHANGED << stack, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

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
