---- MODULE MultiProcedureClosure_P_diamond_four ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
begin
F:call Second(a); R:call Third(a); O:return;
end procedure;
procedure Second(b=0)
begin
G:call Fourth(b); H:return;
end procedure;
procedure Third(c=0)
begin
I:call Fourth(c); J:return;
end procedure;
procedure Fourth(d=0)
begin
K:x:=d; N:return;
end procedure;
process One = 1
begin
L:call First(1); E:skip;
end process;
process Two = 2
begin
S:call Fourth(2); T:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f5f563a0" /\ chksum(tla) = "44e21417")
VARIABLES pc, x, stack, a, b, c, d

vars == << pc, x, stack, a, b, c, d >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Procedure Second *)
        /\ b = [ self \in ProcSet |-> 0]
        (* Procedure Third *)
        /\ c = [ self \in ProcSet |-> 0]
        (* Procedure Fourth *)
        /\ d = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "S"]

F(self) == /\ pc[self] = "F"
           /\ /\ b' = [b EXCEPT ![self] = a[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "Second",
                                                       pc        |->  "R",
                                                       b         |->  b[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "G"]
           /\ UNCHANGED << x, a, c, d >>

R(self) == /\ pc[self] = "R"
           /\ /\ c' = [c EXCEPT ![self] = a[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "Third",
                                                       pc        |->  "O",
                                                       c         |->  c[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "I"]
           /\ UNCHANGED << x, a, b, d >>

O(self) == /\ pc[self] = "O"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, b, c, d >>

First(self) == F(self) \/ R(self) \/ O(self)

G(self) == /\ pc[self] = "G"
           /\ /\ d' = [d EXCEPT ![self] = b[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "Fourth",
                                                       pc        |->  "H",
                                                       d         |->  d[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "K"]
           /\ UNCHANGED << x, a, b, c >>

H(self) == /\ pc[self] = "H"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a, c, d >>

Second(self) == G(self) \/ H(self)

I(self) == /\ pc[self] = "I"
           /\ /\ d' = [d EXCEPT ![self] = c[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "Fourth",
                                                       pc        |->  "J",
                                                       d         |->  d[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "K"]
           /\ UNCHANGED << x, a, b, c >>

J(self) == /\ pc[self] = "J"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ c' = [c EXCEPT ![self] = Head(stack[self]).c]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a, b, d >>

Third(self) == I(self) \/ J(self)

K(self) == /\ pc[self] = "K"
           /\ x' = d[self]
           /\ pc' = [pc EXCEPT ![self] = "N"]
           /\ UNCHANGED << stack, a, b, c, d >>

N(self) == /\ pc[self] = "N"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ d' = [d EXCEPT ![self] = Head(stack[self]).d]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a, b, c >>

Fourth(self) == K(self) \/ N(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, b, c, d >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, b, c, d >>

One == L \/ E

S == /\ pc[2] = "S"
     /\ /\ d' = [d EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "Fourth",
                                              pc        |->  "T",
                                              d         |->  d[2] ] >>
                                          \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "K"]
     /\ UNCHANGED << x, a, b, c >>

T == /\ pc[2] = "T"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, b, c, d >>

Two == S \/ T

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two
           \/ (\E self \in ProcSet:  \/ First(self) \/ Second(self) \/ Third(self)
                                     \/ Fourth(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
