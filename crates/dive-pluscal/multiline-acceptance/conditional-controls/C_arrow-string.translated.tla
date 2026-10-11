---- MODULE C_arrow_string ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=<<"->",
  "<-">>; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "e5bbccd9" /\ chksum(tla) = "ab6b0ff7")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                           <<"->",
             "<-">>
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
