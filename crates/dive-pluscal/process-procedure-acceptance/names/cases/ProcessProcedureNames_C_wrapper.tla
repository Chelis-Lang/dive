---- MODULE ProcessProcedureNames_C_wrapper ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure ProcSet(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call ProcSet(2); E:skip; }
} *)
====
