---- MODULE Macro_P_duplicate_params ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a, a) begin x := a; end macro; begin L: Put(2,3); E: skip; end algorithm; *)
====
