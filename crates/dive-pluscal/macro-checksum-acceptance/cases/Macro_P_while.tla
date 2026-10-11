---- MODULE Macro_P_while ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin while v<a do v:=v+1; end while; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
