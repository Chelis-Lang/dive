---- MODULE RecursiveHeader_C_self_two_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0,k=0) { R1:if (n>0) { call Recur(n-1,k+1); } else { x:=k; };R2:return; }; { L:call Recur(2,0); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "156b0137" /\ chksum(tla) = "8a2e369c")
VARIABLES pc, x, stack, n, k

vars == << pc, x, stack, n, k >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ k = 0
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ /\ k' = k+1
                    /\ n' = n-1
                    /\ stack' = << [ procedure |->  "Recur",
                                     pc        |->  "R2",
                                     n         |->  n,
                                     k         |->  k ] >>
                                 \o stack
                 /\ pc' = "R1"
                 /\ x' = x
            ELSE /\ x' = k
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, n, k >>

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ n' = Head(stack).n
      /\ k' = Head(stack).k
      /\ stack' = Tail(stack)
      /\ x' = x

Recur == R1 \/ R2

L == /\ pc = "L"
     /\ /\ k' = 0
        /\ n' = 2
        /\ stack' = << [ procedure |->  "Recur",
                         pc        |->  "E",
                         n         |->  n,
                         k         |->  k ] >>
                     \o stack
     /\ pc' = "R1"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, n, k >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
