---- MODULE MacroExtra_P_record_field ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := [v |-> a, a |-> a].a; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
