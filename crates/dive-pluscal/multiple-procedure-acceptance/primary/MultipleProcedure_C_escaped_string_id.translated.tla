---- MODULE MultipleProcedure_C_escaped_string_id ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = "\\p")

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "8ddcde2f" /\ chksum(tla) = "ae329f37")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {"\\p"} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = "\\p" -> "L"
                                        [] self = 2 -> "M"]

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

L == /\ pc["\\p"] = "L"
     /\ /\ a' = [a EXCEPT !["\\p"] = 1]
        /\ stack' = [stack EXCEPT !["\\p"] = << [ procedure |->  "First",
                                                  pc        |->  "E",
                                                  a         |->  a["\\p"] ] >>
                                              \o stack["\\p"]]
     /\ pc' = [pc EXCEPT !["\\p"] = "F"]
     /\ x' = x

E == /\ pc["\\p"] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT !["\\p"] = "Done"]
     /\ UNCHANGED << x, stack, a >>

One == L \/ E

M == /\ pc[2] = "M"
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

Two == M \/ N

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
