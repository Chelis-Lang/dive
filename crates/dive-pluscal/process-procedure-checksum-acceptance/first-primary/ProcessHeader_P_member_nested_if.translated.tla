---- MODULE ProcessHeader_P_member_nested_if ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker \in {1,2}
begin
L:if TRUE then call First(2); else x:=0; end if; E:skip;
end process;

end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "2a334ccf" /\ chksum(tla) = "932d58e8")
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
           /\ x' = a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ IF TRUE
                 THEN /\ /\ a' = [a EXCEPT ![self] = 2]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                                  pc        |->  "E",
                                                                  a         |->  a[self] ] >>
                                                              \o stack[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = 0
                      /\ pc' = [pc EXCEPT ![self] = "E"]
                      /\ UNCHANGED << stack, a >>

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
