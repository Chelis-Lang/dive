---- MODULE ApiAwait ----
EXTENDS Integers,TLC
(*
--algorithm A
variable x=0; begin L: await /\ TRUE
 /\ x=0; E: skip; end algorithm;
*)
====
