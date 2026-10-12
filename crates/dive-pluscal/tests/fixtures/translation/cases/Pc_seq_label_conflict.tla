---- MODULE Pc_seq_label_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable L=0; begin L: L := 1; E: skip; end algorithm;
*)
====
