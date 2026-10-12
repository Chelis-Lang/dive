---- MODULE MultiProcedureClosure_C_direct_reverse_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
{ F:call Second(a); R:return; };
procedure Second(b=0)
{ G:call Third(b); H:return; };
procedure Third(c=0)
{ I:x:=c; J:return; };
process (One = 1)
{ L:call Third(1); E:call First(2); U:skip; }
process (Two = 2)
{ S:call Third(2); T:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "389d32cc" /\ chksum(tla) = "41139a4")
VARIABLES pc, x, stack, a, b, c

vars == << pc, x, stack, a, b, c >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Procedure Second *)
        /\ b = [ self \in ProcSet |-> 0]
        (* Procedure Third *)
        /\ c = [ self \in ProcSet |-> 0]
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
           /\ UNCHANGED << x, a, c >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, b, c >>

First(self) == F(self) \/ R(self)

G(self) == /\ pc[self] = "G"
           /\ /\ c' = [c EXCEPT ![self] = b[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "Third",
                                                       pc        |->  "H",
                                                       c         |->  c[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "I"]
           /\ UNCHANGED << x, a, b >>

H(self) == /\ pc[self] = "H"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a, c >>

Second(self) == G(self) \/ H(self)

I(self) == /\ pc[self] = "I"
           /\ x' = c[self]
           /\ pc' = [pc EXCEPT ![self] = "J"]
           /\ UNCHANGED << stack, a, b, c >>

J(self) == /\ pc[self] = "J"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ c' = [c EXCEPT ![self] = Head(stack[self]).c]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a, b >>

Third(self) == I(self) \/ J(self)

L == /\ pc[1] = "L"
     /\ /\ c' = [c EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "Third",
                                              pc        |->  "E",
                                              c         |->  c[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "I"]
     /\ UNCHANGED << x, a, b >>

E == /\ pc[1] = "E"
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "U",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, b, c >>

U == /\ pc[1] = "U"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, b, c >>

One == L \/ E \/ U

S == /\ pc[2] = "S"
     /\ /\ c' = [c EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "Third",
                                              pc        |->  "T",
                                              c         |->  c[2] ] >>
                                          \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "I"]
     /\ UNCHANGED << x, a, b >>

T == /\ pc[2] = "T"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, b, c >>

Two == S \/ T

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two
           \/ (\E self \in ProcSet: First(self) \/ Second(self) \/ Third(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
