---- MODULE Pc_seq_c_parallel_swap ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=1,y=2; { L: x:=y || y:=x; E: skip; }
*)
====
