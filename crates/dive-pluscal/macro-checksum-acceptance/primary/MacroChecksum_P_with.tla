---- MODULE MacroChecksum_P_with ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin with k=a do v:=k; end with; end macro; begin L: Put(x,1); E:skip; end algorithm; *)
====
