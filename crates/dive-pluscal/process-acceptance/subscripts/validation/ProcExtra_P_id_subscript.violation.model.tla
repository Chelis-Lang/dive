---- MODULE ProcExtra_P_id_subscript ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; process P =1 variables x = <<0,0>>; begin L: x[1]:=1; E: g:=x[1]; end process; end algorithm; *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == /\ g = 0
        /\ x = << 0 , 0 >>
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ x' = [x EXCEPT ![ 1 ] = 1]
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ g' = x [ 1 ]
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ x' = x

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
