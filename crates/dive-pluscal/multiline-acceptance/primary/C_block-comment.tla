---- MODULE C_block_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=1 + (* α
 β *) 2; E:skip; } } *)
====
