---- MODULE MultipleProcedure_C_procedure_process_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(One=0)

{ F:x:=One; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
