---- MODULE Pc_seq_member_initial ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x \in 0..2; begin L: x := x+1; E: skip; end algorithm;
*)
====
