---- MODULE ProcessProcedure_C_process_name_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(Worker=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
