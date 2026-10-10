---- MODULE Pc_seq_c_native_assert ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0;  L: x:=1; assert x=0; E: skip; }
*)
====
