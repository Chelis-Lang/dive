---- MODULE MacroLexical_C_tilde ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A { variable x=0; macro Put(v,a) { v:=a; }; { L: Put(x,"~"); E: skip; } } *)
====
