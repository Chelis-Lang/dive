---- MODULE ProcessHeader_C_member_ordinary ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:x:=1; R:return; };
process (Worker \in {1,2})
{ L:call First(); E:skip; }

} *)
\* BEGIN TRANSLATION (chksum(pcal) = "e89ddc4b" /\ chksum(tla) = "86500f51")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ x = 0
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = 1
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ stack' = [stack EXCEPT ![self] = << [ procedure |->  "First",
                                                    pc        |->  "E" ] >>
                                                \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack >>

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
