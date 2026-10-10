---- MODULE ProcessProcedure_C_repeat_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(2); E:call First(3); Z:skip; }
} *)
====
