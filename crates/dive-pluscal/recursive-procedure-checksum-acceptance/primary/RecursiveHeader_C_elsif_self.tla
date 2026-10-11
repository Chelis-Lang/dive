---- MODULE RecursiveHeader_C_elsif_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n=0) { x:=1; } else if (n>0) { call Recur(n-1); } else { x:=0; };R2:return; }; { L:call Recur(2); E:skip; } } *)
====
