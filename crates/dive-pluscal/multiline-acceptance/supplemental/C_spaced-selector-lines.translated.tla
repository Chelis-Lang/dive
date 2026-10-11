---- MODULE C_spaced_selector_lines ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x = <<0,0>>,y=0; { L:x[1 +
 0]:=2; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "20d02cd0" /\ chksum(tla) = "6ba5f26c")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = <<0,0>>
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT !                                                 [1 +
                        0] = 2]
     /\ pc' = "E"
     /\ y' = y

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
