---- MODULE MultiProcedureClosure_P_mutual_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=0)
begin
G:call First(b); H:return;
end procedure;
process One = 1
begin
L:call First(1); E:skip;
end process;
process Two = 2
begin
S:call Second(2); T:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "76df2d0e" /\ chksum(tla) = "8450b210")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Procedure Second *)
        /\ b = [ self \in ProcSet |-> 0]
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
           /\ UNCHANGED << x, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, b >>

First(self) == F(self) \/ R(self)

G(self) == /\ pc[self] = "G"
           /\ /\ a' = [a EXCEPT ![self] = b[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                       pc        |->  "H",
                                                       a         |->  a[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, b >>

H(self) == /\ pc[self] = "H"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a >>

Second(self) == G(self) \/ H(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, b >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

One == L \/ E

S == /\ pc[2] = "S"
     /\ /\ b' = [b EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "Second",
                                              pc        |->  "T",
                                              b         |->  b[2] ] >>
                                          \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "G"]
     /\ UNCHANGED << x, a >>

T == /\ pc[2] = "T"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

Two == S \/ T

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two
           \/ (\E self \in ProcSet: First(self) \/ Second(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
