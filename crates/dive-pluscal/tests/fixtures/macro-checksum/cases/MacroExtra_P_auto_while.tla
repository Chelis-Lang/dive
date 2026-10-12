---- MODULE MacroExtra_P_auto_while ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin while v<a do v:=v+1; end while; end macro; begin Put(x, 1); skip; end algorithm; *)
====
