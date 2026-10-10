---- MODULE MultipleProcedure_C_label_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ L:call First(2); N:skip; }
} *)
====
