---- MODULE MacroScope_P_qualified_formal ----
EXTENDS Integers, TLC
---- MODULE Helper ----
a == 2
====
I == INSTANCE Helper
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:=I!a; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
====
