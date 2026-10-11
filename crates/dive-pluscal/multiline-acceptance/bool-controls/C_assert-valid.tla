---- MODULE C_assert_valid ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:assert (y=0)
  = TRUE; E:skip; } } *)
====
