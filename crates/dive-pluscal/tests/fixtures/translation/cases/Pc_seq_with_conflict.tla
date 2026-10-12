---- MODULE Pc_seq_with_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: with x=2 do skip; end with; E: skip; end algorithm;
*)
====
