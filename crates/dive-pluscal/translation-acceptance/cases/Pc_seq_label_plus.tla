---- MODULE Pc_seq_label_plus ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L+: x:=1; E: skip; end algorithm;
*)
====
