---- MODULE C_fair_layout_crlf ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:await /\ TRUE
      /\ y=0; E:skip; } } *)
====
