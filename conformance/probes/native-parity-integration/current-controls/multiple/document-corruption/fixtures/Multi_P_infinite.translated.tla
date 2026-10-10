---- MODULE Multi_P_infinite ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: while TRUE do x:=(x+1)%2; end while; end process;  process Q =2 variable y=0; begin M: while TRUE do y:=(y+1)%2; end while; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "bc0d2d34" /\ chksum(tla) = "9cbd63ce")
VARIABLES g, x, y

vars == << g, x, y >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0

P == /\ x' = (x+1)%2
     /\ UNCHANGED << g, y >>

Q == /\ y' = (y+1)%2
     /\ UNCHANGED << g, x >>

Next == P \/ Q

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
