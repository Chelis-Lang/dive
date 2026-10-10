---- MODULE Pc_seq_unicode_subscript ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable f=[a ↦ <<0,0>>]; begin L: f.a[1]:=1; E: skip; end algorithm;
*)
====
