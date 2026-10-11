---- MODULE C_assert ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0; { L: assert x=0; x:=1; } } *)
====
