---- MODULE Macro_P_auto_label ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; begin Put(x,1); skip; end algorithm; *)
====
