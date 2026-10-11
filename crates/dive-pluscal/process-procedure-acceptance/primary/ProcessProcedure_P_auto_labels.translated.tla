---- MODULE ProcessProcedure_P_auto_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
x:=a; return;
end procedure;

process Worker = 1

begin
call First(2); skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "eefe2032" /\ chksum(tla) = "95aa0e47")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_3(self) == /\ pc[self] = "Lbl_3"
               /\ x' = a[self]
               /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
               /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
               /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]

First(self) == Lbl_3(self)

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ /\ a' = [a EXCEPT ![1] = 2]
            /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "First",
                                                  pc        |->  "Lbl_2",
                                                  a         |->  a[1] ] >>
                                              \o stack[1]]
         /\ pc' = [pc EXCEPT ![1] = "Lbl_3"]
         /\ x' = x

Lbl_2 == /\ pc[1] = "Lbl_2"
         /\ TRUE
         /\ pc' = [pc EXCEPT ![1] = "Done"]
         /\ UNCHANGED << x, stack, a >>

Worker == Lbl_1 \/ Lbl_2

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
