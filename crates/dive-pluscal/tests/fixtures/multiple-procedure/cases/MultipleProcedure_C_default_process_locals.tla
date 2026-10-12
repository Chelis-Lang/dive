---- MODULE MultipleProcedure_C_default_process_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)
variable y;
{ L:call First(1); E:skip; }
process (Two = 2)
variable z;
{ M:call First(2); N:skip; }
} *)
====
