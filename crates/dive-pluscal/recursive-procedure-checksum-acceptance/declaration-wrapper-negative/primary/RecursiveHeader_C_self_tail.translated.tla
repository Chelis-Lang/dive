---- MODULE RecursiveHeader_C_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n>0) { call Recur(n-1); return; } else { x:=x+1; return; }; }; L:call Recur(2); E:skip; } *)
====
