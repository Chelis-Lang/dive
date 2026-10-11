---- MODULE RecursiveHeader_C_with_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n>0) { with (w=n-1) { call Recur(w); }; } else { x:=x+1; };R2:return; }; { L:call Recur(2); E:skip; } } *)
====
