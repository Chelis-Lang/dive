---- MODULE DefineGuide_C ----
EXTENDS Integers
(*
--algorithm A {
 variable x=0;
 define { Step(a) == a+1 }
 { L:x:=Step(x); E:skip; }
}
*)
====
