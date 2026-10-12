---- MODULE ProcessProcedure_C_member_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker \in 2..1)

{ L:call First(2); E:skip; }
} *)
====
