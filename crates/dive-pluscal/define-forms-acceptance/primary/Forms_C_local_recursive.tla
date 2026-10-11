---- MODULE Forms_C_local_recursive ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 RECURSIVE F(_) LOCAL F(a)==IF a=0 THEN 0 ELSE F(a-1)
 }
{ L:y:=F(2); E:skip; }
}
*)
====
