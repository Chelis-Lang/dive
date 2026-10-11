---- MODULE P_shape_nested_if ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: if x=0 then if x=1 then x:=2; else x:=1; end if; else skip; end if; end algorithm; *)
====
