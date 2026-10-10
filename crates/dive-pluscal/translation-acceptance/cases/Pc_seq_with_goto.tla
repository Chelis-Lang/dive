---- MODULE Pc_seq_with_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: with k=2 do goto E; end with; E: skip; end algorithm;
*)
====
