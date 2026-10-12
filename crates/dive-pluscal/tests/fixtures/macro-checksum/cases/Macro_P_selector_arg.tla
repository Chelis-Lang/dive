---- MODULE Macro_P_selector_arg ----
EXTENDS Integers, TLC
(* --algorithm A variables x=[i \in 1..2 |-> 0], y=0; macro Put(v, a) begin v[a]:=a+1; end macro; begin L: Put(x,1); E: skip; end algorithm; *)
====
