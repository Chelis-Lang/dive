---- MODULE Pc_seq_parallel_swap ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=1, y=2; begin L: x := y || y := x; E: skip; end algorithm;
*)
====
