---- MODULE Pc_seq_either_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: either goto E; or x:=1; end either; E: skip; end algorithm;
*)
====
