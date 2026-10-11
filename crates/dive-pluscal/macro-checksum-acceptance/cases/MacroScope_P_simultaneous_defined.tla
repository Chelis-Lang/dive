---- MODULE MacroScope_P_simultaneous_defined ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(a,b) begin x:=<<a,b>>; end macro; begin L: Put(b,3); E: skip; end algorithm; *)
====
