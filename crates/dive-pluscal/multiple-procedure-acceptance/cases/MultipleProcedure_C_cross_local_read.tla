---- MODULE MultipleProcedure_C_cross_local_read ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)
variable y=1;
{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(y); N:skip; }
} *)
====
