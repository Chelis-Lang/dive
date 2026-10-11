---- MODULE RecursiveHeader_C_mutual_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Other(m=0) { O1:if (m>0) { call Recur(m-1); } else { x:=x+1; };O2:return; };
procedure Recur(n=0) { R1:if (n>0) { call Other(n-1); } else { x:=x+1; };R2:return; }; { L:call Recur(3); E:skip; } } *)
====
