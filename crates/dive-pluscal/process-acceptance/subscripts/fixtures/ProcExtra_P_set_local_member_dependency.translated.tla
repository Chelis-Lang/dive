---- MODULE ProcExtra_P_set_local_member_dependency ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0,y\in{x}; begin L: g:=y; E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "57236b15" /\ chksum(tla) = "6ff4451a")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ y \in [{1,2} -> {x[CHOOSE self \in  {1,2} : TRUE]}]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = y[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << x, y >>

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << g, x, y >>

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
