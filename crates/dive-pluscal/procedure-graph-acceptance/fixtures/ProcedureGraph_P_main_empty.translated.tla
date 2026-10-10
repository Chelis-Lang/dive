---- MODULE ProcedureGraph_P_main_empty ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin

end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "af81cb01" /\ chksum(tla) = "d6d7d937")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 1
        /\ stack = << >>
        /\ pc = "null"

F == /\ pc = "F"
     /\ x' = a
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, b >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = b
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, b >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

Second == S \/ T

Next == First \/ Second

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
