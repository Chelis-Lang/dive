---- MODULE MultipleProcedure_C_local_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=self;
{ F:x:=x+u; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "134a8f75" /\ chksum(tla) = "9982ccd8")
VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ u = [ self \in ProcSet |-> self]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x+u[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, u >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ u' = [u EXCEPT ![self] = Head(stack[self]).u]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              u         |->  u[1],
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ u' = [u EXCEPT ![1] = self]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, u >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ /\ a' = [a EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "First",
                                              pc        |->  "N",
                                              u         |->  u[2],
                                              a         |->  a[2] ] >>
                                          \o stack[2]]
     /\ u' = [u EXCEPT ![2] = self]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, u >>

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
