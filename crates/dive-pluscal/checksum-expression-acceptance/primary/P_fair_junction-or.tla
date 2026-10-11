---- MODULE P_fair_junction_or ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:await \/ FALSE \/ y=0; E:skip; end algorithm; *)
====
