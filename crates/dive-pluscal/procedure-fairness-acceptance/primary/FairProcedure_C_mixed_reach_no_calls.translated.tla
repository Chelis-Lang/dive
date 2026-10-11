---- MODULE FairProcedure_C_mixed_reach_no_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
fair process (One = 1)

{ L:x:=x+1; E:skip; }
fair + process (Two \in {2,3})

{ M:x:=x+2; N:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "aa5f95d3" /\ chksum(tla) = "73cf0f26")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Procedure Second *)
        /\ b = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self \in {2,3} -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x+a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, b >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, b >>

First(self) == F(self) \/ R(self)

S(self) == /\ pc[self] = "S"
           /\ x' = x+b[self]
           /\ pc' = [pc EXCEPT ![self] = "T"]
           /\ UNCHANGED << stack, a, b >>

T(self) == /\ pc[self] = "T"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a >>

Second(self) == S(self) \/ T(self)

L == /\ pc[1] = "L"
     /\ x' = x+1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << stack, a, b >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

One == L \/ E

M(self) == /\ pc[self] = "M"
           /\ x' = x+2
           /\ pc' = [pc EXCEPT ![self] = "N"]
           /\ UNCHANGED << stack, a, b >>

N(self) == /\ pc[self] = "N"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, b >>

Two(self) == M(self) \/ N(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One
           \/ (\E self \in ProcSet: First(self) \/ Second(self))
           \/ (\E self \in {2,3}: Two(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(One)
        /\ \A self \in {2,3} : SF_vars(Two(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
