---- MODULE DefineGuide_P ----
EXTENDS Integers
(*
--algorithm A
 variable x=0;
 define Step(a) == a+1 end define;
 begin L:x:=Step(x); E:skip; end algorithm;
*)
====
