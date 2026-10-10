---- MODULE ProcessProcedure_C_undefined_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=Missing;
{ F:x:=u; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
