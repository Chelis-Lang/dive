---- MODULE MacroScope_P_record_selector ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:= [a |-> a].a; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
====
