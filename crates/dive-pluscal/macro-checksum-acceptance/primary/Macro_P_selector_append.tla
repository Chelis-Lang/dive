---- MODULE Macro_P_selector_append ----
EXTENDS Integers, TLC
(* --algorithm A variables x=[i \in 1..2 |-> [j \in 1..2 |-> 0]], y=0; macro Put(v, a) begin v[2]:=a; end macro; begin L: Put(x[1],2); E: skip; end algorithm; *)
====
