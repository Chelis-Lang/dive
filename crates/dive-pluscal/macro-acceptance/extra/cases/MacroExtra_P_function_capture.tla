---- MODULE MacroExtra_P_function_capture ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := [a \in {1,2} |-> a]; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
