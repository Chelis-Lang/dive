---- MODULE MacroFreeProcessName ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; macro M(v) begin v:=w; end macro; process P=1 variable z=0; begin L: M(z); end process; process Q=2 variable w=0; begin K: skip; end process; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "43002725" /\ chksum(tla) = "6d11b1be")
VARIABLES pc, x, z, w

vars == << pc, x, z, w >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ x = 0
        (* Process P *)
        /\ z = 0
        (* Process Q *)
        /\ w = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "K"]

L == /\ pc[1] = "L"
     /\ z' = w
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, w >>

P == L

K == /\ pc[2] = "K"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, z, w >>

Q == K

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
