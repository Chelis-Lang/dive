---- MODULE MacroScope_C_function_shadow ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=[a \in {1,2} |-> a]; }; { L: Put(x,2); E: skip; } } *)
====
