---- MODULE MultipleProcedure_P_label_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1

begin
L:call First(1); E:skip;
end process;
process Two = 2

begin
L:call First(2); N:skip;
end process;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f9ec3452" /\ chksum(tla) = "79a458ea")
\* Label L of process One at line 13 col 3 changed to L_
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L_"
                                        [] self = 2 -> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = x+a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L_ == /\ pc[1] = "L_"
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

One == L_ \/ E

L == /\ pc[2] = "L"
     /\ /\ a' = [a EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |->  "First",
                                              pc        |->  "N",
                                              a         |->  a[2] ] >>
                                          \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Two == L \/ N

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two
           \/ (\E self \in ProcSet: First(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
