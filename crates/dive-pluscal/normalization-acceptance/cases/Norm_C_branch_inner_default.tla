---- MODULE Norm_C_branch_inner_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { if (x=0) { x:=1; x:=2; } else { skip; }; skip; } } *)
====
