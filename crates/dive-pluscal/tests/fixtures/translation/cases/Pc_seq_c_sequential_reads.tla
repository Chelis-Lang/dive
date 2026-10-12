---- MODULE Pc_seq_c_sequential_reads ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variables x=0, y=0; { L: x:=x+1; y:=x; E: skip; }
*)
====
