---- MODULE Norm_C_branch_label ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0,y=0; { if (x=0) { x:=1; } else { y:=2; }; x:=2; } } *)
====
