---- MODULE RecursiveHeader_P_with_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur(n=0) begin R1:if n>0 then with w=n-1 do call Recur(w); end with; else x:=x+1; end if;R2:return; end procedure;
begin L:call Recur(2); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4b3d60ad" /\ chksum(tla) = "69e64de3")
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
            THEN /\ LET w == n-1 IN
                      /\ /\ n' = w
                         /\ stack' = << [ procedure |->  "Recur",
                                          pc        |->  "R2",
                                          n         |->  n ] >>
                                      \o stack
                      /\ pc' = "R1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, n >>

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
