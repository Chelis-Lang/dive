---- MODULE MacroOrigin_P_while ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin while v<a do v:=v+1; end while; end macro; macro Outer(v,a) begin Put(v,a); end macro; begin Outer(x, 1); skip; end algorithm; *)
====
