---- MODULE ProcessProcedure_C_self_arg ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker \in {1,2})

{ L:call First(self); E:skip; }
} *)
====
