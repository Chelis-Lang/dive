---- MODULE Pc_seq_indexed_update ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable f=<<0,0>>; begin L: f[1]:=1; E: skip; end algorithm;
*)
====
