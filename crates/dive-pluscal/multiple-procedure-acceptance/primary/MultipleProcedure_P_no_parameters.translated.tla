---- MODULE MultipleProcedure_P_no_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:x:=x+1; R:return;
end procedure;
process One = 1

begin
L:call First(); E:skip;
end process;
process Two = 2

begin
M:call First(); N:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "98b50c26" /\ chksum(tla) = "d9a575d7")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x+1
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                           pc        |->  "E" ] >>
                                       \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "First",
                                           pc        |->  "N" ] >>
                                       \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack >>

Two == M \/ N

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
