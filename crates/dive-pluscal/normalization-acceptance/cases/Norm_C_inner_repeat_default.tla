---- MODULE Norm_C_inner_repeat_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { while (x<2) { x:=x+1; x:=x; }; } } *)
====
