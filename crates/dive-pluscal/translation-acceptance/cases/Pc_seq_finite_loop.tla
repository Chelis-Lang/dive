---- MODULE Pc_seq_finite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while x<2 do x := x+1; end while; E: skip; end algorithm;
*)
====
