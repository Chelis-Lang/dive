---- MODULE MacroLexical_P_space ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L: Put(x," "); E: skip; end algorithm; *)
====
