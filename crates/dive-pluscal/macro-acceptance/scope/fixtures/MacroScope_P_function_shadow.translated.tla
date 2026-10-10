---- MODULE MacroScope_P_function_shadow ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:=[a \in {1,2} |-> a]; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1d0e4ddc" /\ chksum(tla) = "3e66d812")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == (* Global variables *)
        /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [2 \in {1,2} |-> 2]
     /\ pc' = "E"
     /\ b' = b

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
