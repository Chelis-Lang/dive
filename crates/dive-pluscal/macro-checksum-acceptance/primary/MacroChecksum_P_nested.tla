---- MODULE MacroChecksum_P_nested ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; macro Twice(v,a) begin Put(v,a+1); end macro; begin L:Twice(x,1); E:skip; end algorithm; *)
====
