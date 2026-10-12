---- MODULE Fair_C_infinite_weak_minus ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{1,2}) variables x=0; { L-: while (TRUE) { x:=(x+1)%2; }; } } *)
====
