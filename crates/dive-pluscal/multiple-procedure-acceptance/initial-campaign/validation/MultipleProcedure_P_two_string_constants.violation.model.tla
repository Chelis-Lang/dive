---- MODULE MultipleProcedure_P_two_string_constants ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = "z"

begin
L:call First(1); E:skip;
end process;
process Two = "a"

begin
M:call First(2); N:skip;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {"z"} \cup {"a"}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = "z" -> "L" [] self = "a" -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x + a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc["z"] = "L"
     /\ /\ a' = [a EXCEPT !["z"] = 1]
        /\ stack' = [stack EXCEPT !["z"] = << [ procedure |-> "First", pc |-> "E", a |-> a["z"] ] >> \o stack["z"]]
     /\ pc' = [pc EXCEPT !["z"] = "F"]
     /\ x' = x

E == /\ pc["z"] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["z"] = "Done"]
     /\ UNCHANGED << x, stack, a >>

One == L \/ E

M == /\ pc["a"] = "M"
     /\ /\ a' = [a EXCEPT !["a"] = 2]
        /\ stack' = [stack EXCEPT !["a"] = << [ procedure |-> "First", pc |-> "N", a |-> a["a"] ] >> \o stack["a"]]
     /\ pc' = [pc EXCEPT !["a"] = "F"]
     /\ x' = x

N == /\ pc["a"] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["a"] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Two == M \/ N

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
