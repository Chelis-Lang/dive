---- MODULE Pc_seq_infinite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while TRUE do x := (x+1) % 2; end while; end algorithm;
*)
====
