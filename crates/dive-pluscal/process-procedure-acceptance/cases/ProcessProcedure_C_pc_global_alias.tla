---- MODULE ProcessProcedure_C_pc_global_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0,pc=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
