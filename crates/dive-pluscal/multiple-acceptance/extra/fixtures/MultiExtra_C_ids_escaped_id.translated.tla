---- MODULE MultiExtra_C_ids_escaped_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; LE: g:=x; }  process (Q =2) variable y=0; { M: y:=2; ME: g:=y; }  process (R ="a\tb") variable z=0; { N: z:=3; NE: g:=z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "aeecf161" /\ chksum(tla) = "7ff0f117")
VARIABLES pc, g, x, y, z

vars == << pc, g, x, y, z >>

ProcSet == {1} \cup {2} \cup {"a\tb"}

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
                                        [] self = "a\tb" -> "N"]

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

N == /\ pc["a\tb"] = "N"
     /\ z' = 3
     /\ pc' = [pc EXCEPT !["a\tb"] = "NE"]
     /\ UNCHANGED << g, x, y >>

NE == /\ pc["a\tb"] = "NE"
      /\ g' = z
      /\ pc' = [pc EXCEPT !["a\tb"] = "Done"]
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
