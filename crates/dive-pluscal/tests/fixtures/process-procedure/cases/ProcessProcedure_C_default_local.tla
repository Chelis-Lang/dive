---- MODULE ProcessProcedure_C_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u;
{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
