---- MODULE C_token_record ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x = [a |-> 0]; { L:x.a:=1; E:skip; } } *)
====
