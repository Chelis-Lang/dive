---- MODULE Norm_C_with_repeat_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { with (k=1) { x:=k; x:=2; }; } } *)
====
