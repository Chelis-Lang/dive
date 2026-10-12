---- MODULE Macro_P_index_lhs ----
EXTENDS Integers, TLC
(* --algorithm A variables x=[i \in 1..2 |-> 0], y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x[1],2); E: skip; end algorithm; *)
====
