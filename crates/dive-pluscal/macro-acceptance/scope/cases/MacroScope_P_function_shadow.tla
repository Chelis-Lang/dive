---- MODULE MacroScope_P_function_shadow ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:=[a \in {1,2} |-> a]; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
====
