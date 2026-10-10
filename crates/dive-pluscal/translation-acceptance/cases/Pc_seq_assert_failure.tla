---- MODULE Pc_seq_assert_failure ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x:=1; assert x=0; E: skip; end algorithm;
*)
====
