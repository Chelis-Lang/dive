---- MODULE MultiProcedureClosure_C_diamond_four ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
{ F:call Second(a); R:call Third(a); O:return; };
procedure Second(b=0)
{ G:call Fourth(b); H:return; };
procedure Third(c=0)
{ I:call Fourth(c); J:return; };
procedure Fourth(d=0)
{ K:x:=d; N:return; };
process (One = 1)
{ L:call First(1); E:skip; }
process (Two = 2)
{ S:call Fourth(2); T:skip; }
} *)
====
