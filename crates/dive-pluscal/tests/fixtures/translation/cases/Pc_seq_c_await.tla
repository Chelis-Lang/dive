---- MODULE Pc_seq_c_await ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; { L: x:=1; await x=1; E: skip; }
*)
====
