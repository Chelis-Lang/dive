---- MODULE CallUnknown ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put() begin P:return; end procedure; begin L:call Missing(); end algorithm; *)
====
