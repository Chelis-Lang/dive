---- MODULE MultiProcedureClosure_C_diamond_reverse_declarations ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Fourth(d=0)
{ K:x:=d; N:return; };
procedure Third(c=0)
{ I:call Fourth(c); J:return; };
procedure Second(b=0)
{ G:call Fourth(b); H:return; };
procedure First(a=0)
{ F:call Second(a); R:call Third(a); O:return; };
process (One = 1)
{ L:call First(1); E:skip; }
process (Two = 2)
{ S:call Fourth(2); T:skip; }
} *)
====
