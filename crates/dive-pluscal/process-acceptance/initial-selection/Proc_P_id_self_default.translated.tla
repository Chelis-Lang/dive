---- MODULE Proc_P_id_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=self; begin L: g:=x; E: x:=self+1; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "c672db41" /\ chksum(tla) = "bc2e8d19")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = self
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ x' = self+1
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
