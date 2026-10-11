---- MODULE Macro_P_utf8 ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := <<"λ",a>>; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
