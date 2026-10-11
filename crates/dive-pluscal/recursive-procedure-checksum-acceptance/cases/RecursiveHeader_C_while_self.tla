---- MODULE RecursiveHeader_C_while_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:while (n>0) { WC:call Recur(n-1); WA:n:=0; };R2:return; }; L:call Recur(2); E:skip; } *)
====
