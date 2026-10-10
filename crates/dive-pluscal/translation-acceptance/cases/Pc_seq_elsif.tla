---- MODULE Pc_seq_elsif ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: if x=0 then x:=1; elsif x=1 then x:=2; else x:=3; end if; E: skip; end algorithm;
*)
====
