---- MODULE ProcessProcedure_C_constant_string ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = "p")

{ L:call First(2); E:skip; }
} *)
====
