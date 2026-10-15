---- MODULE ProcessSelfTail ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure Recur(a=0,c=0) variable u=a+c; begin F: call Recur(a-1,a); return; end procedure; process Worker=1 begin L: call Recur(2,3); E: skip; end process; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "c8502391" /\ chksum(tla) = "90f3c88e")
VARIABLES pc, x, stack, a, c, u

vars == << pc, x, stack, a, c, u >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ a = [ self \in ProcSet |-> 0]
        /\ c = [ self \in ProcSet |-> 0]
        /\ u = [ self \in ProcSet |-> a[self]+c[self]]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ /\ a' = [a EXCEPT ![self] = a[self]-1]
              /\ c' = [c EXCEPT ![self] = a[self]]
           /\ u' = [u EXCEPT ![self] = a'[self]+c'[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, stack >>

Recur(self) == F(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ c' = [c EXCEPT ![1] = 3]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |->  "Recur",
                                              pc        |->  "E",
                                              u         |->  u[1],
                                              a         |->  a[1],
                                              c         |->  c[1] ] >>
                                          \o stack[1]]
     /\ u' = [u EXCEPT ![1] = a'[1]+c'[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, c, u >>

Worker == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker
           \/ (\E self \in ProcSet: Recur(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
