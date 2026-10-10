---- MODULE Pc_seq_nested_with ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: with a=1,b \in {1,2} do x:=a+b; end with; E: skip; end algorithm;
*)
====
