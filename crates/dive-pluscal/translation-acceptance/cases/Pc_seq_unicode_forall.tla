---- MODULE Pc_seq_unicode_forall ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: x:=1; print ∀ k ∈ {1}: k=x; E: skip; end algorithm;
*)
====
