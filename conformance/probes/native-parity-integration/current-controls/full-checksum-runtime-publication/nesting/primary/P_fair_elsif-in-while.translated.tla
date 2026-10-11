---- MODULE P_fair_elsif_in_while ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:while x<2 do if x=0 then x:=1; elsif x=1 then x:=2; end if; end while; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "44b2bc86")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x<2
           THEN /\ IF x=0
                      THEN /\ x' = 1
                      ELSE /\ IF x=1
                                 THEN /\ x' = 2
                                 ELSE /\ TRUE
                                      /\ x' = x
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
