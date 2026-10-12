---- MODULE RecursiveHeader_C_mutual_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:if (n>0) { call Other(n-1);return; } else { x:=x+1; };R2:return; };
procedure Other(m=0) { O1:if (m>0) { call Recur(m-1);return; } else { x:=x+1; };O2:return; }; { L:call Recur(3); E:skip; } } *)
====
