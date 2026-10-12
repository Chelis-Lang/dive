---- MODULE P_junction_nested ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:await /\ (y=0 \/ y=1)
      /\ TRUE; E:skip; end algorithm; *)
====
