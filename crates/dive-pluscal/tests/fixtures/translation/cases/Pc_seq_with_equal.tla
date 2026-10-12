---- MODULE Pc_seq_with_equal ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: with k=2 do x:=k; end with; E: skip; end algorithm;
*)
====
