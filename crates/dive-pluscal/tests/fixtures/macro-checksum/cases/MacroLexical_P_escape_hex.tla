---- MODULE MacroLexical_P_escape_hex ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L: Put(x,"\x7f"); E: skip; end algorithm; *)
====
