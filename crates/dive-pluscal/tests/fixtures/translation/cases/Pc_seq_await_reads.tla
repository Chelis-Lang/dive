---- MODULE Pc_seq_await_reads ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x:=1; await x=1; E: skip; end algorithm;
*)
====
