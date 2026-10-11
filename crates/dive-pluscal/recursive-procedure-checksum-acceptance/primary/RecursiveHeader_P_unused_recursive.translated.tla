---- MODULE RecursiveHeader_P_unused_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:call Recur(n); R2:return; end procedure;
begin L:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "339d10cb" /\ chksum(tla) = "fde2a15c")
VARIABLES pc, x, stack, n

vars == << pc, x, stack, n >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ /\ n' = n
         /\ stack' = << [ procedure |->  "Recur",
                          pc        |->  "R2",
                          n         |->  n ] >>
                      \o stack
      /\ pc' = "R1"
      /\ x' = x

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ n' = Head(stack).n
      /\ stack' = Tail(stack)
      /\ x' = x

Recur == R1 \/ R2

L == /\ pc = "L"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, n >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
