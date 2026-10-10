---- MODULE MultiBound_C_escaped_overlap ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; process (P ="a\tb") variable x=0; { L: x:=1; E: g:=x; } process (Q ="a\tb") variable y=0; { M: y:=2; F: g:=y; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "1110c0f1" /\ chksum(tla) = "6d68cec1")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {"a\tb"} \cup {"a\tb"}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = "a\tb" -> "L"
                                        [] self = "a\tb" -> "M"]

L == /\ pc["a\tb"] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT !["a\tb"] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc["a\tb"] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT !["a\tb"] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

M == /\ pc["a\tb"] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT !["a\tb"] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc["a\tb"] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT !["a\tb"] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
