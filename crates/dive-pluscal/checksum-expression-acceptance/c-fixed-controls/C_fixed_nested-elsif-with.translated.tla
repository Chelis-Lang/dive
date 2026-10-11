---- MODULE C_fixed_nested_elsif_with ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(a=1,b=2){if(x=0){x:=a;}else{if(x=1){x:=b;}else{skip;}}}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f29f255" /\ chksum(tla) = "d5fbc43f")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
          LET b == 2 IN
            IF x=0
               THEN /\ x' = a
               ELSE /\ IF x=1
                          THEN /\ x' = b
                          ELSE /\ TRUE
                               /\ x' = x
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
