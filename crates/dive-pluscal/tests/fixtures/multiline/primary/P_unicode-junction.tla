---- MODULE P_unicode_junction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:await ∧ TRUE
  ∧ y=0; E:skip; end algorithm; *)
====
