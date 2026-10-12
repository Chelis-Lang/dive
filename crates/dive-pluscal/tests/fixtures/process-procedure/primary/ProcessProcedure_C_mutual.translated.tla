---- MODULE ProcessProcedure_C_mutual ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){ call Second(a-1); }else{ x:=a; }; R:return; };
procedure Second(b=0) { S:if(b>0){ call First(b-1); }else{ x:=b; }; T:return; };
process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "770558c2" /\ chksum(tla) = "419f322f")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        (* Procedure Second *)
        /\ b = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self]>0
                 THEN /\ /\ b' = [b EXCEPT ![self] = a[self]-1]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "Second",
                                                                  pc        |->  "R",
                                                                  b         |->  b[self] ] >>
                                                              \o stack[self]]
                      /\ pc' = [pc EXCEPT ![self] = "S"]
                      /\ x' = x
                 ELSE /\ x' = a[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << stack, b >>
           /\ a' = a

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, b >>

First(self) == F(self) \/ R(self)

S(self) == /\ pc[self] = "S"
           /\ IF b[self]>0
                 THEN /\ /\ a' = [a EXCEPT ![self] = b[self]-1]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                                  pc        |->  "T",
                                                                  a         |->  a[self] ] >>
                                                              \o stack[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = b[self]
                      /\ pc' = [pc EXCEPT ![self] = "T"]
                      /\ UNCHANGED << stack, a >>
           /\ b' = b

T(self) == /\ pc[self] = "T"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a >>

Second(self) == S(self) \/ T(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 2]
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

Worker == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker
           \/ (\E self \in ProcSet: First(self) \/ Second(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
