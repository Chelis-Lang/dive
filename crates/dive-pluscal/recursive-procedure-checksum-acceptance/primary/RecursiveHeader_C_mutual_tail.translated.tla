---- MODULE RecursiveHeader_C_mutual_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n>0) { call Other(n-1);return; } else { x:=x+1; };R2:return; };
procedure Other(m=0) { O1:if (m>0) { call Recur(m-1);return; } else { x:=x+1; };O2:return; }; { L:call Recur(3); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "6cd00378" /\ chksum(tla) = "75259eab")
VARIABLES pc, x, stack, n, m

vars == << pc, x, stack, n, m >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        (* Procedure Other *)
        /\ m = 0
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ /\ m' = n-1
                    /\ stack' = << [ procedure |->  "Other",
                                     pc        |->  Head(stack).pc,
                                     m         |->  m ] >>
                                 \o Tail(stack)
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

O1 == /\ pc = "O1"
      /\ IF m>0
            THEN /\ /\ n' = m-1
                    /\ stack' = << [ procedure |->  "Recur",
                                     pc        |->  Head(stack).pc,
                                     n         |->  n ] >>
                                 \o Tail(stack)
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
     /\ UNCHANGED << x, stack, n, m >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ Other \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
