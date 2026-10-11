---- MODULE MacroScope_C_quantifier_shadow ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=\A a \in {1,2}: a>0; }; { L: Put(x,2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "8781c5f0" /\ chksum(tla) = "2689d5f6")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == (* Global variables *)
        /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (\A 2 \in {1,2}: 2>0)
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
