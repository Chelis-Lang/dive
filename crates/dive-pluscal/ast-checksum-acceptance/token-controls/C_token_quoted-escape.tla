---- MODULE C_token_quoted_escape ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x="a\"b"; { L:x:="c\\d"; E:skip; } } *)
====
