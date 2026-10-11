---- MODULE P_separator_only_comma ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:with , do x:=a; end with; E:skip; end algorithm; *)
====
