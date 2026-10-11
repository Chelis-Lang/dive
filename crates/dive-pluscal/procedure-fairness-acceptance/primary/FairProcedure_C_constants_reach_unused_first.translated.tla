---- MODULE FairProcedure_C_constants_reach_unused_first ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Third(c=0)

{ U:x:=x+c; V:return; };
procedure First(a=0)

{ F:x:=x+a; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two = 2)

{ M:call Second(2); N:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "60afe8c9" /\ chksum(tla) = "80826f15")
VARIABLES pc, x, stack, c, a, b

vars == << pc, x, stack, c, a, b >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Third *)
        /\ c = [ self \in ProcSet |-> 0]
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Procedure Second *)
        /\ b = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

U(self) == /\ pc[self] = "U"
           /\ x' = x+c[self]
           /\ pc' = [pc EXCEPT ![self] = "V"]
           /\ UNCHANGED << stack, c, a, b >>

V(self) == /\ pc[self] = "V"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ c' = [c EXCEPT ![self] = Head(stack[self]).c]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a, b >>

Third(self) == U(self) \/ V(self)

F(self) == /\ pc[self] = "F"
           /\ x' = x+a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, c, a, b >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, c, b >>

First(self) == F(self) \/ R(self)

S(self) == /\ pc[self] = "S"
           /\ x' = x+b[self]
           /\ pc' = [pc EXCEPT ![self] = "T"]
           /\ UNCHANGED << stack, c, a, b >>

T(self) == /\ pc[self] = "T"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, c, a >>

Second(self) == S(self) \/ T(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, c, b >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, c, a, b >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ /\ b' = [b EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "Second",
                                              pc        |->  "N",
                                              b         |->  b[2] ] >>
                                          \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "S"]
     /\ UNCHANGED << x, c, a >>

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, c, a, b >>

Two == M \/ N

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two
           \/ (\E self \in ProcSet: Third(self) \/ First(self) \/ Second(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(One) /\ WF_vars(First(1))
        /\ SF_vars(Two) /\ SF_vars(Second(2))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
