---- MODULE ProcessProcedure_C_member_initializer ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u \in {1,2};
{ F:x:=u; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
