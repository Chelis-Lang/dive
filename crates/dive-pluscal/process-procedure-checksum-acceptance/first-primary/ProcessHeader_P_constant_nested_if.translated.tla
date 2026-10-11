---- MODULE ProcessHeader_P_constant_nested_if ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:if TRUE then call First(2); else x:=0; end if; E:skip;
end process;

end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8ac35ff4" /\ chksum(tla) = "3da6e589")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ IF TRUE
           THEN /\ /\ a' = [a EXCEPT ![1] = 2]
                   /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                                         pc        |->  "E",
                                                         a         |->  a[1] ] >>
                                                     \o stack[1]]
                /\ pc' = [pc EXCEPT ![1] = "F"]
                /\ x' = x
           ELSE /\ x' = 0
                /\ pc' = [pc EXCEPT ![1] = "E"]
                /\ UNCHANGED << stack, a >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Worker == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
