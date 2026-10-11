---- MODULE RecursiveHeader_C_zero_param_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur() { R1:call Recur(); return; }; { L:call Recur(); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "3db87d9a" /\ chksum(tla) = "7ffbecd3")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

R1 == /\ pc = "R1"
      /\ 
      /\ pc' = "R1"
      /\ UNCHANGED << x, stack >>

Recur == R1

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "Recur",
                      pc        |->  "E" ] >>
                  \o stack
     /\ pc' = "R1"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Recur \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
