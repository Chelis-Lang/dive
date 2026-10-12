---- MODULE MacroExtra_C_unicode_actual ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(x, ∀ k ∈ {1}: TRUE); E: skip; } } *)
====
