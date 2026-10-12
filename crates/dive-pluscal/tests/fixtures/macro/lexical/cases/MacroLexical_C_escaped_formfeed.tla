---- MODULE MacroLexical_C_escaped_formfeed ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A { variable x=0; macro Put(v,a) { v:=a; }; { L: Put(x,"\f"); E: skip; } } *)
====
