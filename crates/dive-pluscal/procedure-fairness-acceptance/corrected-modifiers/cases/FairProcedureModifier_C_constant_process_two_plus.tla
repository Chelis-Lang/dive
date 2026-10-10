---- MODULE FairProcedureModifier_C_constant_process_two_plus ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
fair process (One = 1)

{ L: + call First(1); E: + skip; }
} *)
====
