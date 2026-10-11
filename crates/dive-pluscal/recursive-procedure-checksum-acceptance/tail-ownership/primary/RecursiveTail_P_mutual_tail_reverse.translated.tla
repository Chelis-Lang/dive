---- MODULE RecursiveTail_P_mutual_tail_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Other(m=0) variables q=m; begin O1:if m>0 then call Recur(m-1);return; else x:=x+1; end if;O2:return; end procedure; procedure Recur(n=0) variables v=n; begin R1:if n>0 then call Other(n-1);return; else x:=x+1; end if;R2:return; end procedure;

begin L:call Recur(3); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8a407392" /\ chksum(tla) = "e0a73d33")
VARIABLES pc, x, stack, m, q, n, v

vars == << pc, x, stack, m, q, n, v >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Other *)
        /\ m = 0
        /\ q = m
        (* Procedure Recur *)
        /\ n = 0
        /\ v = n
        /\ stack = << >>
        /\ pc = "L"

O1 == /\ pc = "O1"
      /\ IF m>0
            THEN /\ /\ n' = m-1
                    /\ q' = Head(stack).q
                    /\ stack' = << [ procedure |->  "Recur",
                                     pc        |->  Head(stack).pc,
                                     v         |->  v,
                                     n         |->  n ] >>
                                 \o Tail(stack)
                 /\ v' = n'
                 /\ pc' = "R1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "O2"
                 /\ UNCHANGED << stack, q, n, v >>
      /\ m' = m

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ m' = Head(stack).m
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, n, v >>

Other == O1 \/ O2

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ /\ m' = n-1
                    /\ stack' = << [ procedure |->  "Other",
                                     pc        |->  Head(stack).pc,
                                     q         |->  q,
                                     m         |->  m ] >>
                                 \o Tail(stack)
                    /\ v' = Head(stack).v
                 /\ q' = m'
                 /\ pc' = "O1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, m, q, v >>
      /\ n' = n

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ v' = Head(stack).v
      /\ n' = Head(stack).n
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, m, q >>

Recur == R1 \/ R2

L == /\ pc = "L"
     /\ /\ n' = 3
        /\ stack' = << [ procedure |->  "Recur",
                         pc        |->  "E",
                         v         |->  v,
                         n         |->  n ] >>
                     \o stack
     /\ v' = n'
     /\ pc' = "R1"
     /\ UNCHANGED << x, m, q >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, m, q, n, v >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Other \/ Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
