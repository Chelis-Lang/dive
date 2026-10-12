---- MODULE MultipleProcedure_C_procedure_process_name ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure One(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call One(1); E:skip; }
process (Two = 2)

{ M:call One(2); N:skip; }
} *)
====
