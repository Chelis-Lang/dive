---- MODULE MacroScope_C_quantifier_shadow ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=\A a \in {1,2}: a>0; }; { L: Put(x,2); E: skip; } } *)
====
