---- MODULE MultipleProcedure_C_duplicate_process_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)
variable y=1;
{ L:call First(1); E:skip; }
process (Two = 2)
variable y=2;
{ M:call First(2); N:skip; }
} *)
====
