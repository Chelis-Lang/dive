---- MODULE MacroLexical_C_escape_hex ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A { variable x=0; macro Put(v,a) { v:=a; }; { L: Put(x,"\x7f"); E: skip; } } *)
====
