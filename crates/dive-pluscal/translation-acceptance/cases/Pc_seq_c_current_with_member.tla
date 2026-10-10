---- MODULE Pc_seq_c_current_with_member ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: with (k \in {1,2}) {x:=k;} E: skip; }}
*)
====
