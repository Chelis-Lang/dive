---- MODULE P_junction_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:await /\ TRUE (* inside *)
      /\ y=0; E:skip; end algorithm; *)
====
