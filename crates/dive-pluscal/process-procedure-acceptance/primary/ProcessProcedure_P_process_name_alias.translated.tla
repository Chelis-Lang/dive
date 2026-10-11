---- MODULE ProcessProcedure_P_process_name_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(Worker=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "85f5c71f" /\ chksum(tla) = "e3ad5aa2")
\* Process Worker at line 11 col 1 changed to Worker_
VARIABLES pc, x, stack, Worker

vars == << pc, x, stack, Worker >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ Worker = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = a
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, Worker >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ Worker' = [Worker EXCEPT ![self] = Head(stack[self]).Worker]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ Worker' = [Worker EXCEPT ![1] = 2]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              Worker    |->  Worker[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, Worker >>

Worker_ == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker_
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
