---- MODULE MultiExtra_P_ids_labels ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin x:=1; LE: g:=x; end process;  process Q =2 variable y=0; begin y:=1; ME: g:=y; end process;  process R =3 variable z=0; begin z:=1; NE: g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5d4af006" /\ chksum(tla) = "ee9c9b6b")
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
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "Lbl_1"
                                        [] self = 2 -> "Lbl_2"
                                        [] self = 3 -> "Lbl_3"]

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ x' = 1
         /\ pc' = [pc EXCEPT ![1] = "LE"]
         /\ UNCHANGED << g, y, z >>

LE == /\ pc[1] = "LE"
      /\ g' = x
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, y, z >>

P == Lbl_1 \/ LE

Lbl_2 == /\ pc[2] = "Lbl_2"
         /\ y' = 1
         /\ pc' = [pc EXCEPT ![2] = "ME"]
         /\ UNCHANGED << g, x, z >>

ME == /\ pc[2] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == Lbl_2 \/ ME

Lbl_3 == /\ pc[3] = "Lbl_3"
         /\ z' = 1
         /\ pc' = [pc EXCEPT ![3] = "NE"]
         /\ UNCHANGED << g, x, y >>

NE == /\ pc[3] = "NE"
      /\ g' = z
      /\ pc' = [pc EXCEPT ![3] = "Done"]
      /\ UNCHANGED << x, y, z >>

R == Lbl_3 \/ NE

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ R
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
