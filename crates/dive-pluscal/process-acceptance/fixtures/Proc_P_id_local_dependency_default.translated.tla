---- MODULE Proc_P_id_local_dependency_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0,y=x+1; begin L: g:=y; E: x:=g; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "16e72e20" /\ chksum(tla) = "a8334289")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        /\ y = x+1
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << x, y >>

E == /\ pc[1] = "E"
     /\ x' = g
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, y >>

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
