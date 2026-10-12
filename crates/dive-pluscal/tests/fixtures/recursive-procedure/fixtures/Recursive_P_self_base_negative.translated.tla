---- MODULE Recursive_P_self_base_negative ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=x+1; end if; R:return;
end procedure;
begin
L:call First(0-1); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4e0fde8d" /\ chksum(tla) = "b33f8e89")
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
                /\ x' = x
           ELSE /\ x' = x+1
                /\ pc' = "R"
                /\ UNCHANGED << stack, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 0-1
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
