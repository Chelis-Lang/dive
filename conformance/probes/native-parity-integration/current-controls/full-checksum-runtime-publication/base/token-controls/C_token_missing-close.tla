---- MODULE C_token_missing_close ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x = <<0,1>; { L:x:=1; E:skip; } } *)
====
