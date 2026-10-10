---- MODULE Macro_P_zero ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put() begin x:=1; end macro; begin L: Put(); E: skip; end algorithm; *)
====
