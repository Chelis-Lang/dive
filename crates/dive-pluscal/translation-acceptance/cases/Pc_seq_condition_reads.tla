---- MODULE Pc_seq_condition_reads ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; begin L: x := 1; if x=1 then y:=1; else y:=2; end if; E: skip; end algorithm;
*)
====
