---- MODULE ProcessProcedure_C_process_label_modifier ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L+:call First(2); E:skip; }
} *)
====
