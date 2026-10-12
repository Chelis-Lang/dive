---- MODULE Pc_seq_label_assert ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: assert x=0; E: skip; end algorithm;
*)
====
