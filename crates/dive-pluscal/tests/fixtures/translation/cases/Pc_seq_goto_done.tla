---- MODULE Pc_seq_goto_done ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x := 1; goto Done; E: skip; end algorithm;
*)
====
