---- MODULE MultiProcedureClosure_C_mutual_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
{ F:call Second(a); R:return; };
procedure Second(b=0)
{ G:call First(b); H:return; };
process (One = 1)
{ L:call First(1); E:skip; }
process (Two = 2)
{ S:call Second(2); T:skip; }
} *)
====
