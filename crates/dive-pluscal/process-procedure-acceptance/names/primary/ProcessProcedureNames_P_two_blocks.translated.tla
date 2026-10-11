---- MODULE ProcessProcedureNames_P_two_blocks ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
process Other = 2
begin
O:call First(1); Z:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4ed8f446" /\ chksum(tla) = "234e24a2")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "O"]

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
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Worker == L \/ E

O == /\ pc[2] = "O"
     /\ /\ a' = [a EXCEPT ![2] = 1]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "First",
                                              pc        |->  "Z",
                                              a         |->  a[2] ] >>
                                          \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

Z == /\ pc[2] = "Z"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Other == O \/ Z

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker \/ Other
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
