---- MODULE Fair_P_infinite_weak_plain ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; fair process P \in{1,2} variables x=0; begin L: while TRUE do x:=(x+1)%2; end while; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "6e3153c3" /\ chksum(tla) = "52d38c4c")
VARIABLES g, x

vars == << g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]

P(self) == /\ x' = [x EXCEPT ![self] = (x[self]+1)%2]
           /\ g' = g

Next == (\E self \in {1,2}: P(self))

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : WF_vars(P(self))

\* END TRANSLATION 
====
