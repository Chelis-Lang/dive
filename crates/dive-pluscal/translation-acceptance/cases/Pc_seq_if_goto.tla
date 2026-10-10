---- MODULE Pc_seq_if_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: if x=0 then goto E; else skip; end if; E: skip; end algorithm;
*)
====
