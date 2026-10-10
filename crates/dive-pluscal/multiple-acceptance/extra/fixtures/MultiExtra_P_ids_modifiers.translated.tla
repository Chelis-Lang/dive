---- MODULE MultiExtra_P_ids_modifiers ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P =1 variable x=0; begin L: + x:=1; LE: - g:=x; end process; fair process Q =2 variable y=0; begin M: + y:=2; ME: - g:=y; end process; fair process R =3 variable z=0; begin N: + z:=3; NE: - g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "483eda19" /\ chksum(tla) = "6ff97c39")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == {1} \cup {2} \cup {3}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0
        (* Process R *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"
                                        [] self = 3 -> "N"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "LE"]
     /\ UNCHANGED << g, y, z >>

LE == /\ pc[1] = "LE"
      /\ g' = x
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, y, z >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "ME"]
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[2] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ z' = 3
     /\ pc' = [pc EXCEPT ![3] = "NE"]
     /\ UNCHANGED << g, x, y >>

NE == /\ pc[3] = "NE"
      /\ g' = z
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

R == N \/ NE

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars((pc[1] # "LE") /\ P) /\ SF_vars(L)
        /\ WF_vars((pc[2] # "ME") /\ Q) /\ SF_vars(M)
        /\ WF_vars((pc[3] # "NE") /\ R) /\ SF_vars(N)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
