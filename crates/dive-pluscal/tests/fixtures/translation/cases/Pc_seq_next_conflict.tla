---- MODULE Pc_seq_next_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable Next=0; begin L: Next := 1; E: skip; end algorithm;
*)
====
