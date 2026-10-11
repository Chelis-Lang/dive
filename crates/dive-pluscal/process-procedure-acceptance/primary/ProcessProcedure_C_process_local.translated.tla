---- MODULE ProcessProcedure_C_process_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)
variable y=1;
{ L:call First(y); E:y:=x; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "3c4a24fe" /\ chksum(tla) = "4a792a81")
VARIABLES pc, x, stack, a, y

vars == << pc, x, stack, a, y >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Process Worker *)
        /\ y = 1
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, y >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, y >>

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = y]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, y >>

E == /\ pc[1] = "E"
     /\ y' = x
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
