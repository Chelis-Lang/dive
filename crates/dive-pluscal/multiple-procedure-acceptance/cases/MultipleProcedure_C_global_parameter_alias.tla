---- MODULE MultipleProcedure_C_global_parameter_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(x=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
