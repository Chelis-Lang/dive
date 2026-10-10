---- MODULE Pc_seq_pc_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable pc=0; begin L: pc := 1; E: skip; end algorithm;
*)
====
