---- MODULE MultiProcedureClosure_C_self_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
{ F:call First(a); R:return; };
procedure Second(b=0)
{ G:x:=b; H:return; };
process (One = 1)
{ L:call First(1); E:skip; }
process (Two = 2)
{ S:call Second(2); T:skip; }
} *)
====
