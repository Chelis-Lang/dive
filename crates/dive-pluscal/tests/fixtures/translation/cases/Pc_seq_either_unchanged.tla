---- MODULE Pc_seq_either_unchanged ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; begin L: either x:=1; or y:=2; end either; E: skip; end algorithm;
*)
====
