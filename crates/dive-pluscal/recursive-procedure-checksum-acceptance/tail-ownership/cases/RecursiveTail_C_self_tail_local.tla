---- MODULE RecursiveTail_C_self_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) variables v=n; { R1:if (n>0) { call Recur(n-1); return; } else { x:=x+v+1; return; }; }; { L:call Recur(2); E:skip; } } *)
====
