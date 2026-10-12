---- MODULE ProcessProcedure_C_self_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=self;
{ F:x:=u; R:return; };

process (Worker \in {1,2})

{ L:call First(2); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "c2bac1ff" /\ chksum(tla) = "d46ab106")
VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ u = [ self \in ProcSet |-> self]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = u[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, u >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ u' = [u EXCEPT ![self] = Head(stack[self]).u]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                       pc        |->  "E",
                                                       u         |->  u[self],
                                                       a         |->  a[self] ] >>
                                                   \o stack[self]]
           /\ u' = [u EXCEPT ![self] = self]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, u >>

Worker(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in ProcSet: First(self))
           \/ (\E self \in {1,2}: Worker(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
