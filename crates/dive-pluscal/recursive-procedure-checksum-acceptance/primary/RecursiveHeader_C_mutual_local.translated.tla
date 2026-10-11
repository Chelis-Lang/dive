---- MODULE RecursiveHeader_C_mutual_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) variables v=n; { R1:if (n>0) { call Other(n-1); } else { x:=x+1; };R2:return; };
procedure Other(m=0) variables q=m; { O1:if (m>0) { call Recur(m-1); } else { x:=x+1; };O2:return; }; { L:call Recur(3); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "9cc6ebff" /\ chksum(tla) = "d47ebf99")
VARIABLES pc, x, stack, n, v, m, q

vars == << pc, x, stack, n, v, m, q >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ v = n
        (* Procedure Other *)
        /\ m = 0
        /\ q = m
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ /\ m' = n-1
                    /\ stack' = << [ procedure |->  "Other",
                                     pc        |->  "R2",
                                     q         |->  q,
                                     m         |->  m ] >>
                                 \o stack
                 /\ q' = m'
                 /\ pc' = "O1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, m, q >>
      /\ UNCHANGED << n, v >>

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ v' = Head(stack).v
      /\ n' = Head(stack).n
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, m, q >>

Recur == R1 \/ R2

O1 == /\ pc = "O1"
      /\ IF m>0
            THEN /\ /\ n' = m-1
                    /\ stack' = << [ procedure |->  "Recur",
                                     pc        |->  "O2",
                                     v         |->  v,
                                     n         |->  n ] >>
                                 \o stack
                 /\ v' = n'
                 /\ pc' = "R1"
                 /\ x' = x
            ELSE /\ x' = x+1
                 /\ pc' = "O2"
                 /\ UNCHANGED << stack, n, v >>
      /\ UNCHANGED << m, q >>

O2 == /\ pc = "O2"
      /\ pc' = Head(stack).pc
      /\ q' = Head(stack).q
      /\ m' = Head(stack).m
      /\ stack' = Tail(stack)
      /\ UNCHANGED << x, n, v >>

Other == O1 \/ O2

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
     /\ UNCHANGED << x, stack, n, v, m, q >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ Other \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
