---- MODULE FairProcedure_C_constant_proc_F_rplus_unfair ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R+:return; };
process (One = 1)

{ L:call First(1); E:skip; }
} *)
====
