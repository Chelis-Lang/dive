---- MODULE MultiExtra_C_ids_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1 || g:=(x+1)%3; LE: skip; }  process (Q =2) variable y=0; { M: y:=2 || g:=(y+1)%3; ME: skip; }  process (R =3) variable z=0; { N: z:=3 || g:=(z+1)%3; NE: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "9cc74ab9" /\ chksum(tla) = "47c9dc73")
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
     /\ /\ g' = (x+1)%3
        /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "LE"]
     /\ UNCHANGED << y, z >>

LE == /\ pc[1] = "LE"
      /\ TRUE
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << g, x, y, z >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ /\ g' = (y+1)%3
        /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "ME"]
     /\ UNCHANGED << x, z >>

ME == /\ pc[2] = "ME"
      /\ TRUE
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << g, x, y, z >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ /\ g' = (z+1)%3
        /\ z' = 3
     /\ pc' = [pc EXCEPT ![3] = "NE"]
     /\ UNCHANGED << x, y >>

NE == /\ pc[3] = "NE"
      /\ TRUE
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << g, x, y, z >>

R == N \/ NE

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
