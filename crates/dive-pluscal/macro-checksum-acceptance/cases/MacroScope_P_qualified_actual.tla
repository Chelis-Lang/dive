---- MODULE MacroScope_P_qualified_actual ----
EXTENDS Integers, TLC
---- MODULE Helper ----
a == 2
====
I == INSTANCE Helper
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:=a; end macro; begin L: Put(x,I!a); E: skip; end algorithm; *)
====
