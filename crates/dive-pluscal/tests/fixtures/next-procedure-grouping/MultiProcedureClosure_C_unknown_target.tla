---- MODULE MultiProcedureClosure_C_unknown_target ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
{ F:call Second(a); R:return; };
procedure Second(b=0)
{ G:call Third(b); H:return; };
procedure Third(c=0)
{ I:x:=c; J:return; };
process (One = 1)
{ L:call Missing(1); E:skip; }
process (Two = 2)
{ S:call Third(2); T:skip; }
} *)
====
