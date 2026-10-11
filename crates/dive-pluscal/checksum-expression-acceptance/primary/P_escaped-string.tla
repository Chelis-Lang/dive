---- MODULE P_escaped_string ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:="left\nright"; E:skip; end algorithm; *)
====
