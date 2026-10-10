---- MODULE Recursive_P_mutual_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Second(b=0)

begin
S:if b>0 then call First(b-1); else x:=1; end if; T:return;
end procedure;
procedure First(a=0)

begin
F:if a>0 then call Second(a-1); else x:=0; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f8132c08" /\ chksum(tla) = "cfe0a13")
VARIABLES pc, x, stack, b, a

vars == << pc, x, stack, b, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Second *)
        /\ b = 0
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

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
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

Second == S \/ T

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
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b >>

First == F \/ R

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
     /\ UNCHANGED << x, stack, b, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Second \/ First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
