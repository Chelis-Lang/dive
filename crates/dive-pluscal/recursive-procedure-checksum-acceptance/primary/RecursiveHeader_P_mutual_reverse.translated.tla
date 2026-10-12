---- MODULE RecursiveHeader_P_mutual_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Other(m=0) begin O1:if m>0 then call Recur(m-1); else x:=x+1; end if;O2:return; end procedure;
procedure Recur(n=0) begin R1:if n>0 then call Other(n-1); else x:=x+1; end if;R2:return; end procedure;
begin L:call Recur(3); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5da9071f" /\ chksum(tla) = "553f7679")
VARIABLES pc, x, stack, m, n

vars == << pc, x, stack, m, n >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Other *)
        /\ m = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ IF m>0
            THEN /\ /\ n' = m-1
                    /\ stack' = << [ procedure |->  "Recur",
                                     pc        |->  "O2",
                                     n         |->  n ] >>
                                 \o stack
                 /\ pc' = "R1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "O2"
                 /\ UNCHANGED << stack, n >>
      /\ m' = m

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ m' = Head(stack).m
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, n >>

Other == O1 \/ O2

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ /\ m' = n-1
                    /\ stack' = << [ procedure |->  "Other",
                                     pc        |->  "R2",
                                     m         |->  m ] >>
                                 \o stack
                 /\ pc' = "O1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, m >>
      /\ n' = n

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ n' = Head(stack).n
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, m >>

Recur == R1 \/ R2

L == /\ pc = "L"
     /\ /\ n' = 3
        /\ stack' = << [ procedure |->  "Recur",
                         pc        |->  "E",
                         n         |->  n ] >>
                     \o stack
     /\ pc' = "R1"
     /\ UNCHANGED << x, m >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, m, n >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Other \/ Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
