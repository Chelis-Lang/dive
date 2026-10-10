---- MODULE ProcExtra_P_id_local_member_dependency ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; process P =1 variables x=0,y\in{x}; begin L: g:=y; E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a6599451" /\ chksum(tla) = "a662f77b")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        /\ y \in {x}
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << x, y >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, x, y >>

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
