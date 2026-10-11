---- MODULE RecursiveHeader_C_self_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) variables v=n; { R1:if (n>0) { call Recur(n-1); } else { x:=x+v+1; };R2:return; }; { L:call Recur(2); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "cb5137d5" /\ chksum(tla) = "583f4397")
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
            THEN /\ /\ n' = n-1
                    /\ stack' = << [ procedure |->  "Recur",
                                     pc        |->  "R2",
                                     v         |->  v,
                                     n         |->  n ] >>
                                 \o stack
                 /\ v' = n'
                 /\ pc' = "R1"
                 /\ x' = x
            ELSE /\ x' = x+v+1
                 /\ pc' = "R2"
                 /\ UNCHANGED << stack, n, v >>

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ v' = Head(stack).v
      /\ n' = Head(stack).n
      /\ stack' = Tail(stack)
      /\ x' = x

Recur == R1 \/ R2

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
