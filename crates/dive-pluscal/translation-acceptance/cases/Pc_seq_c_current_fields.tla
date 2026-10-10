---- MODULE Pc_seq_c_current_fields ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable f=[a|->0,b|->0]; { L: f.a:=1 || f.b:=2; E: skip; }}
*)
====
