---- MODULE P_decl_lines ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x = <<1,
      2>>, y=0; begin L:skip; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a705e2b0" /\ chksum(tla) = "df63dc51")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x =                          <<1,
               2>>
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ TRUE
     /\ pc' = "E"
     /\ UNCHANGED << x, y >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
