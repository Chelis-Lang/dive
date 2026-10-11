---- MODULE Macro_C_string_rhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a) { x := <<"a",a>>; }; { L: Put(2); E: skip; } } *)
====
