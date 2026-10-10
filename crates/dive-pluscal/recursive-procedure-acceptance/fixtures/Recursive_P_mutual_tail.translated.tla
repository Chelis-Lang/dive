---- MODULE Recursive_P_mutual_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call Second(a-1); return; else x:=a; end if; R:return;
end procedure;
procedure Second(b=0)

begin
S:if b>0 then call First(b-1); return; else x:=b; end if; T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f644f489" /\ chksum(tla) = "7f451835")
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
                                    pc        |->  Head(stack).pc,
                                    b         |->  b ] >>
                                \o Tail(stack)
                /\ pc' = "S"
                /\ x' = x
           ELSE /\ x' = a
                /\ pc' = "R"
                /\ UNCHANGED << stack, b >>
     /\ a' = a

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b >>

First == F \/ R

S == /\ pc = "S"
     /\ IF b>0
           THEN /\ /\ a' = b-1
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  Head(stack).pc,
                                    a         |->  a ] >>
                                \o Tail(stack)
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = b
                /\ pc' = "T"
                /\ UNCHANGED << stack, a >>
     /\ b' = b

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

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
