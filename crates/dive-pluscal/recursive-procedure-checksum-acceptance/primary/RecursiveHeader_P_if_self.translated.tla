---- MODULE RecursiveHeader_P_if_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n=0 then x:=x+1; else call Recur(n-1); end if;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "3082d2e" /\ chksum(tla) = "a4de5120")
VARIABLES pc, x, stack, n

vars == << pc, x, stack, n >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ IF n=0
            THEN /\ x' = x+1
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, n >>
            ELSE /\ /\ n' = n-1
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
