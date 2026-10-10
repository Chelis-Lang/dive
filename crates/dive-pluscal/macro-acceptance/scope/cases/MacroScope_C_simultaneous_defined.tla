---- MODULE MacroScope_C_simultaneous_defined ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(a,b) { x:=<<a,b>>; }; { L: Put(b,3); E: skip; } } *)
====
