---- MODULE MacroChecksum_P_nested ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; macro Twice(v,a) begin Put(v,a+1); end macro; begin L:Twice(x,1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "685f7a3" /\ chksum(tla) = "4a203edf")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1+1
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
