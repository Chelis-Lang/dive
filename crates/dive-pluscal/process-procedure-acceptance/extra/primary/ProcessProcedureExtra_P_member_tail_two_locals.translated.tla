---- MODULE ProcessProcedureExtra_P_member_tail_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
begin
F:if a>0 then call First(a-1); return; else x:=u+v; end if; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2); E:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5b2330b2" /\ chksum(tla) = "6e46c2b8")
VARIABLES pc, x, stack, a, u, v

vars == << pc, x, stack, a, u, v >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ u = [ self \in ProcSet |-> a[self]+1]
        /\ v = [ self \in ProcSet |-> u[self]+1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self]>0
                 THEN /\ a' = [a EXCEPT ![self] = a[self]-1]
                      /\ u' = [u EXCEPT ![self] = a'[self]+1]
                      /\ v' = [v EXCEPT ![self] = u'[self]+1]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = u[self]+v[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << a, u, v >>
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ u' = [u EXCEPT ![self] = Head(stack[self]).u]
           /\ v' = [v EXCEPT ![self] = Head(stack[self]).v]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                       pc        |->  "E",
                                                       u         |->  u[self],
                                                       v         |->  v[self],
                                                       a         |->  a[self] ] >>
                                                   \o stack[self]]
           /\ u' = [u EXCEPT ![self] = a'[self]+1]
           /\ v' = [v EXCEPT ![self] = u'[self]+1]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, u, v >>

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
