---- MODULE Inter_C_with_after ----
EXTENDS Naturals,TLC
(* --algorithm A { variables x=0; { with (k=1) { x:=k; }; x:=2; } } *)
====
