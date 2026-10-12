---- MODULE Pc_seq_sequential_reads ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; begin L: x := x+1; y := x; E: skip; end algorithm;
*)
====
