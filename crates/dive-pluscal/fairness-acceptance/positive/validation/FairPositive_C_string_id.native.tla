---- MODULE FairPositive_C_string_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P ="worker") variables x=0; { L: + x:=1; E: - g:=x; } } *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {"worker"}

Init == /\ g = 0
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc["worker"] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT !["worker"] = "E"]
     /\ g' = g

E == /\ pc["worker"] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT !["worker"] = "Done"]
     /\ x' = x

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars((pc["worker"] # "E") /\ P) /\ SF_vars(L)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
