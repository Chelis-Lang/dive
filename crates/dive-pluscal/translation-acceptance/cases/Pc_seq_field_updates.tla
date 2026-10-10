---- MODULE Pc_seq_field_updates ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable f=[a |-> 0, b |-> 0]; begin L: f.a:=1 || f.b:=2; E: skip; end algorithm;
*)
====
