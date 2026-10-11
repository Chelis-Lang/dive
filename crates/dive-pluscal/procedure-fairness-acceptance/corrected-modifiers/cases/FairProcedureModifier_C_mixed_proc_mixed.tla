---- MODULE FairProcedureModifier_C_mixed_proc_mixed ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F: - x:=x+a; R: + return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
process (Two \in {2,3})

{ M:call First(2); N:skip; }
} *)
====
