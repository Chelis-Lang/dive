---- MODULE MacroLexical_C_escape_unicode ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A { variable x=0; macro Put(v,a) { v:=a; }; { L: Put(x,"\u03bb"); E: skip; } } *)
====
