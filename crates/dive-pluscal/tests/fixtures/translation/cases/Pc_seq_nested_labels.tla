---- MODULE Pc_seq_nested_labels ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while x<2 do B: x := x+1; end while; E: skip; end algorithm;
*)
====
