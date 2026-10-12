---- MODULE Pc_seq_c_native_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0;  L: x:=1; goto E; E: goto Done; }
*)
====
