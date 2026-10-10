---- MODULE Pc_seq_repeated_write ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x := 1; x := 2; end algorithm;
*)
====
