---- MODULE Macro_P_while ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin while v<a do v:=v+1; end while; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "df6994d" /\ chksum(tla) = "345b317c")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x < 1
           THEN /\ x' = x + 1
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
