---- MODULE Rewrite_P ----
EXTENDS Integers
(* --algorithm A { procedure R() { L:return; }; { E:call R(); } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "a11255a5" /\ chksum(tla) = "cacc69d3")
VARIABLES pc, stack

vars == << pc, stack >>

Init == /\ stack = << >>
        /\ pc = "E"

L == /\ pc = "L"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)

R == L

E == /\ pc = "E"
     /\ stack' = << [ procedure |->  "R",
                      pc        |->  "Done" ] >>
                  \o stack
     /\ pc' = "L"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == R \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
\* preserved footer
====
