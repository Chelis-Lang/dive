---- MODULE MultiExtra_P_ids_loop ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: while x<2 do x:=x+1; end while; LE: g:=x; end process;  process Q =2 variable y=0; begin M: while y<2 do y:=y+1; end while; ME: g:=y; end process;  process R =3 variable z=0; begin N: while z<2 do z:=z+1; end while; NE: g:=z; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1c2c7f63" /\ chksum(tla) = "4245796e")
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
     /\ IF x<2
           THEN /\ x' = x+1
                /\ pc' = [pc EXCEPT ![1] = "L"]
           ELSE /\ pc' = [pc EXCEPT ![1] = "LE"]
                /\ x' = x
     /\ UNCHANGED << g, y, z >>

LE == /\ pc[1] = "LE"
      /\ g' = x
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ UNCHANGED << x, y, z >>

P == L \/ LE

M == /\ pc[2] = "M"
     /\ IF y<2
           THEN /\ y' = y+1
                /\ pc' = [pc EXCEPT ![2] = "M"]
           ELSE /\ pc' = [pc EXCEPT ![2] = "ME"]
                /\ y' = y
     /\ UNCHANGED << g, x, z >>

ME == /\ pc[2] = "ME"
      /\ g' = y
      /\ pc' = [pc EXCEPT ![2] = "Done"]
      /\ UNCHANGED << x, y, z >>

Q == M \/ ME

N == /\ pc[3] = "N"
     /\ IF z<2
           THEN /\ z' = z+1
                /\ pc' = [pc EXCEPT ![3] = "N"]
           ELSE /\ pc' = [pc EXCEPT ![3] = "NE"]
                /\ z' = z
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

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
