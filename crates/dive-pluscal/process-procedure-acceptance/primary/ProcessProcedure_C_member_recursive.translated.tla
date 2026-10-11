---- MODULE ProcessProcedure_C_member_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){ call First(a-1); }else{ x:=a; }; R:return; };

process (Worker \in {1,2})

{ L:call First(2); E:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "4a0f9b58" /\ chksum(tla) = "2adf64d")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self]>0
                 THEN /\ /\ a' = [a EXCEPT ![self] = a[self]-1]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                                  pc        |->  "R",
                                                                  a         |->  a[self] ] >>
                                                              \o stack[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = a[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                       pc        |->  "E",
                                                       a         |->  a[self] ] >>
                                                   \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a >>

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
