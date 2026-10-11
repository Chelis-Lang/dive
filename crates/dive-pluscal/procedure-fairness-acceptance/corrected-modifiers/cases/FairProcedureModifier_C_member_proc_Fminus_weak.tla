---- MODULE FairProcedureModifier_C_member_proc_Fminus_weak ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F: - x:=x+a; R:return; };
fair process (One \in {1,2})

{ L:call First(1); E:skip; }
} *)
====
