---- MODULE Norm_C_mixed_default_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x,y=0; { L: x:=y; y:=1; E: skip; } } *)
====
