---- MODULE Inter_C_nested_with ----
EXTENDS Naturals,TLC
(* --algorithm A { variables x=0; { with (k=1) { with (j=2) { x:=k+j; }; }; } } *)
====
