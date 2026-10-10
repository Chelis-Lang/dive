---- MODULE Recursive_P_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u;
begin
F:if a>0 then call First(a-1); else x:=a; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8c721260" /\ chksum(tla) = "be1e1298")
CONSTANT defaultInitValue
VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ u = defaultInitValue
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ /\ a' = a-1
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  "R",
                                    u         |->  u,
                                    a         |->  a ] >>
                                \o stack
                /\ u' = defaultInitValue
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = a
                /\ pc' = "R"
                /\ UNCHANGED << stack, a, u >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         u         |->  u,
                         a         |->  a ] >>
                     \o stack
     /\ u' = defaultInitValue
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, u >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
