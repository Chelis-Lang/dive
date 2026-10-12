---- MODULE Fair_C_infinite_unfair_minus ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0;  process (P \in{1,2}) variables x=0; { L-: while (TRUE) { x:=(x+1)%2; }; } } *)
====
