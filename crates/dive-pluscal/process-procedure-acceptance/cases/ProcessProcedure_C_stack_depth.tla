---- MODULE ProcessProcedure_C_stack_depth ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=Len(stack); R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
