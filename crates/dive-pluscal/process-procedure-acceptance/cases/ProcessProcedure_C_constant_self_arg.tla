---- MODULE ProcessProcedure_C_constant_self_arg ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(self); E:skip; }
} *)
====
