---- MODULE FairProcedure_P_mixed_reach_self_recursion ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=x+1; end if; R:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two \in {2,3}

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "55fe50e9" /\ chksum(tla) = "651d14e6")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self \in {2,3} -> "M"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self]>0
                 THEN /\ /\ a' = [a EXCEPT ![self] = a[self]-1]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                                  pc        |->  "R",
                                                                  a         |->  a[self] ] >>
                                                              \o stack[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = x+1
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                              pc        |->  "E",
                                              a         |->  a[1] ] >>
                                          \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a >>

One == L \/ E

M(self) == /\ pc[self] = "M"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                       pc        |->  "N",
                                                       a         |->  a[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

N(self) == /\ pc[self] = "N"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a >>

Two(self) == M(self) \/ N(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One
           \/ (\E self \in ProcSet: First(self))
           \/ (\E self \in {2,3}: Two(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(One) /\ WF_vars(First(1))
        /\ \A self \in {2,3} : SF_vars(Two(self)) /\ SF_vars(First(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
