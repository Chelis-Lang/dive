---- MODULE MacroOrigin_P_assert ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin assert a>0; v:=a; end macro; macro Outer(v,a) begin Put(v,a); end macro; begin Outer(x, 1); skip; end algorithm; *)
====
