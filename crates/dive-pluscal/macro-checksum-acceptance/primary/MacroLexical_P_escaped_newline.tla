---- MODULE MacroLexical_P_escaped_newline ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L: Put(x,"\n"); E: skip; end algorithm; *)
====
