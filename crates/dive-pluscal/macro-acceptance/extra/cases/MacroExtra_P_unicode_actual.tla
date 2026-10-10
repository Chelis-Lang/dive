---- MODULE MacroExtra_P_unicode_actual ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x, ∀ k ∈ {1}: TRUE); E: skip; end algorithm; *)
====
