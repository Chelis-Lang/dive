---- MODULE P_missing_elsif_test ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then x:=1; elsif then skip; end if; E:skip; end algorithm; *)
====
