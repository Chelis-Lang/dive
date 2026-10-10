---- MODULE FairProcedure_P_invalid_no_param_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call First(); return;
end procedure;
fair process One = 1

begin
L:call First(); E:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a17a72a3" /\ chksum(tla) = "9a149c29")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ 
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, stack >>

First(self) == F(self)

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

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(One) /\ WF_vars(First(1))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
