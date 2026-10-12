---- MODULE ProcessProcedure_C_self_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=self;
{ F:x:=u; R:return; };

process (Worker \in {1,2})

{ L:call First(2); E:skip; }
} *)
====
