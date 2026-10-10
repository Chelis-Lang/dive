---- MODULE Pc_seq_vars_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable vars=0; begin L: vars := 1; E: skip; end algorithm;
*)
====
