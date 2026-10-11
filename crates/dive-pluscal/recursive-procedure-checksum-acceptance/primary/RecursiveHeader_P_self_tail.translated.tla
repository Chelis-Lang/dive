---- MODULE RecursiveHeader_P_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n>0 then call Recur(n-1); return; else x:=x+1; return; end if; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "aeda6abc" /\ chksum(tla) = "611d3688")
VARIABLES pc, x, stack, n

vars == << pc, x, stack, n >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ n' = n-1
                 /\ pc' = "R1"
                 /\ UNCHANGED << x, stack >>
            ELSE /\ x' = x+1
                 /\ pc' = Head(stack).pc
                 /\ n' = Head(stack).n
                 /\ stack' = Tail(stack)

Recur == R1

L == /\ pc = "L"
     /\ /\ n' = 2
        /\ stack' = << [ procedure |->  "Recur",
                         pc        |->  "E",
                         n         |->  n ] >>
                     \o stack
     /\ pc' = "R1"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, n >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
