---- MODULE Pc_seq_branch_unchanged ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; begin L: if TRUE then x:=1; else y:=2; end if; E: skip; end algorithm;
*)
====
