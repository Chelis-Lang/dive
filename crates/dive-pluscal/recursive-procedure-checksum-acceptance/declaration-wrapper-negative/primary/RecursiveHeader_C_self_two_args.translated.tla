---- MODULE RecursiveHeader_C_self_two_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0,k=0) { R1:if (n>0) { call Recur(n-1,k+1); } else { x:=k; };R2:return; }; L:call Recur(2,0); E:skip; } *)
====
