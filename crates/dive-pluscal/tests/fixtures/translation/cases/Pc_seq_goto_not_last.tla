---- MODULE Pc_seq_goto_not_last ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: goto E; x:=1; E: skip; end algorithm;
*)
====
