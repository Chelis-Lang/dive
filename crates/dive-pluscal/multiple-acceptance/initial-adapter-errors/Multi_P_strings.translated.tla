---- MODULE Multi_P_strings ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P ="p" variable x=0; begin L: x:=1; E: g:=x; end process;  process Q ="q" variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5a9a24ae" /\ chksum(tla) = "15e8c68a")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {"p"} \cup {"q"}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = "p" -> "L"
                                        [] self = "q" -> "M"]

L == /\ pc["p"] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT !["p"] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc["p"] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT !["p"] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

M == /\ pc["q"] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT !["q"] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc["q"] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT !["q"] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
