---- MODULE Macro_P_bound_capture ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a) begin x := {a \in 1..2 : a>0}; end macro; begin L: Put(2); E: skip; end algorithm; *)
====
