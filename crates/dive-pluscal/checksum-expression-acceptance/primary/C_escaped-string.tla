---- MODULE C_escaped_string ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:="left\nright"; E:skip; } } *)
====
