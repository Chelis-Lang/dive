---- MODULE Inter_C_with_prior ----
EXTENDS Naturals,TLC
(* --algorithm A { variables x=0; { x:=1; with (k=2) { x:=k; }; } } *)
====
