---- MODULE Pc_seq_parenthesized_true ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while (TRUE) do x:=(x+1)%2; end while; E: skip; end algorithm;
*)
====
