---- MODULE RecursiveHeader_C_while_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:while (n>0) { WC:call Recur(n-1); WA:n:=0; };R2:return; }; { L:call Recur(2); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "72c08b7b" /\ chksum(tla) = "6051b1a1")
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
            THEN /\ pc' = "WC"
            ELSE /\ pc' = "R2"
      /\ UNCHANGED << x, stack, n >>

WC == /\ pc = "WC"
      /\ /\ n' = n-1
         /\ stack' = << [ procedure |->  "Recur",
                          pc        |->  "WA",
                          n         |->  n ] >>
                      \o stack
      /\ pc' = "R1"
      /\ x' = x

WA == /\ pc = "WA"
      /\ n' = 0
      /\ pc' = "R1"
      /\ UNCHANGED << x, stack >>

R2 == /\ pc = "R2"
      /\ pc' = Head(stack).pc
      /\ n' = Head(stack).n
      /\ stack' = Tail(stack)
      /\ x' = x

Recur == R1 \/ WC \/ WA \/ R2

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
