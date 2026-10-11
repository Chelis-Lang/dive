---- MODULE P_fair_nested_elsif_with ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:with a=1,b=2 do if x=0 then x:=a; elsif x=1 then x:=b; else skip; end if; end with; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "55c666f5")
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
