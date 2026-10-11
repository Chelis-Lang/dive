---- MODULE C_token_empty ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x = <<>>; { L:x:=<<1>>; E:skip; } } *)
====
