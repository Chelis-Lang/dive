---- MODULE ProcessProcedure_P_constant_string ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = "p"

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5a4571e4" /\ chksum(tla) = "35e927ae")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {"p"}

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

L == /\ pc["p"] = "L"
     /\ /\ a' = [a EXCEPT !["p"] = 2]
        /\ stack' = [stack EXCEPT !["p"] = << [ procedure |->  "First",
                                                pc        |->  "E",
                                                a         |->  a["p"] ] >>
                                            \o stack["p"]]
     /\ pc' = [pc EXCEPT !["p"] = "F"]
     /\ x' = x

E == /\ pc["p"] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["p"] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Worker == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
