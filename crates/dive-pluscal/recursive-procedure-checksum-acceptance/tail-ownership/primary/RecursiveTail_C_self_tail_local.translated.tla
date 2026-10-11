---- MODULE RecursiveTail_C_self_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) variables v=n; { R1:if (n>0) { call Recur(n-1); return; } else { x:=x+v+1; return; }; }; { L:call Recur(2); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "d2271fed" /\ chksum(tla) = "6287dabf")
VARIABLES pc, x, stack, n, v

vars == << pc, x, stack, n, v >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Recur *)
        /\ n = 0
        /\ v = n
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ IF n>0
            THEN /\ n' = n-1
                 /\ v' = n'
                 /\ pc' = "R1"
                 /\ UNCHANGED << x, stack >>
            ELSE /\ x' = x+v+1
                 /\ pc' = Head(stack).pc
                 /\ v' = Head(stack).v
                 /\ n' = Head(stack).n
                 /\ stack' = Tail(stack)

Recur == R1

L == /\ pc = "L"
     /\ /\ n' = 2
        /\ stack' = << [ procedure |->  "Recur",
                         pc        |->  "E",
                         v         |->  v,
                         n         |->  n ] >>
                     \o stack
     /\ v' = n'
     /\ pc' = "R1"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, n, v >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
