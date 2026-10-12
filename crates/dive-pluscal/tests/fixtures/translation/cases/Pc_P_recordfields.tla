---- MODULE Pc_P_recordfields ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A variable x = 0, y; begin
 x := [begin |-> 1, end |-> 2]; assert x.begin = 1;
end algorithm

*)
====
