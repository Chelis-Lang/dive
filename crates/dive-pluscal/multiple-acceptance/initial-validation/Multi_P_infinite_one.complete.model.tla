---- MODULE Multi_P_infinite_one ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: while TRUE do x:=(x+1)%2; end while; end process;  process Q =2 variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)

VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup {2}

Init == /\ g = 0
        /\ x = 0
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ x' = ( x + 1 ) % 2
     /\ pc' = [pc EXCEPT ![1] = "L"]
     /\ UNCHANGED << g, y >>

P == L

M == /\ pc[2] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc[2] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

Next == P \/ Q

Spec == Init /\ [][Next]_vars


====
