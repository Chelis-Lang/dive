---- MODULE Macro_P_newline_arg ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x,
1 + 2); E: skip; end algorithm; *)
====
