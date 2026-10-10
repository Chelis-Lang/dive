---- MODULE MacroProcess_C_free_self ----
EXTENDS Integers, TLC
(* --algorithm A { variable x=0,y=0; macro Put(v,a) { v:=self; }; process (P=1) variable z=0; { L: Put(z,self); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "2fced719" /\ chksum(tla) = "3fd2af7a")
VARIABLES pc, x, y, z

vars == << pc, x, y, z >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        (* Process P *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ z' = self
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << x, y >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y, z >>

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
