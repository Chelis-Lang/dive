---- MODULE Proc_P_id_infinite_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: while TRUE do x:=(x+1)%2; end while; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "9820a89e" /\ chksum(tla) = "320fd72a")
VARIABLES g, x

vars == << g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0

P == /\ x' = (x+1)%2
     /\ g' = g

Next == P

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
