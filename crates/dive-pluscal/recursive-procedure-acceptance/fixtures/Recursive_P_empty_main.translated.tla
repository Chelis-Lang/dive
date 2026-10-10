---- MODULE Recursive_P_empty_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=a; end if; R:return;
end procedure;
begin

end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "55047aea" /\ chksum(tla) = "cd001a1e")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "null"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ /\ a' = a-1
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

Next == First

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
