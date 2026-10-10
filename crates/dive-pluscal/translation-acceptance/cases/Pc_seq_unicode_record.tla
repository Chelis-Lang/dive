---- MODULE Pc_seq_unicode_record ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x:=1; print [x ↦ x]; E: skip; end algorithm;
*)
====
