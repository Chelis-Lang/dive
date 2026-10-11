---- MODULE MultipleProcedure_C_generated_procset_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(ProcSet=0)

{ F:x:=ProcSet; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
