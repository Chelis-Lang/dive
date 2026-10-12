---- MODULE Macro_P_field_lhs ----
EXTENDS Integers, TLC
(* --algorithm A variables x=[a |-> 0], y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x.a,2); E: skip; end algorithm; *)
====
