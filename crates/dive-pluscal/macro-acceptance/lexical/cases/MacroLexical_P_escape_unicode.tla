---- MODULE MacroLexical_P_escape_unicode ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L: Put(x,"\u03bb"); E: skip; end algorithm; *)
====
