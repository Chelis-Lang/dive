---- MODULE RecursiveHeader_C_if_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n=0) { x:=x+1; } else { call Recur(n-1); };R2:return; }; L:call Recur(2); E:skip; } *)
====
