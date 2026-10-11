---- MODULE Ownership_C_second_algorithm ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro M(a) { x:=a; }; { L:M(1); E:skip; } } --algorithm B variable y=0; begin Q:skip; end algorithm; *)
====
