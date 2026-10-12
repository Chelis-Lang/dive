---- MODULE P_token_junction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L:await /\ TRUE /\ x=0; E:skip; end algorithm; *)
====
