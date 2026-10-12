---- MODULE ProcessProcedure_C_member_proc_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=self; R:return; };

process (Worker \in {1,2})

{ L:call First(2); E:skip; }
} *)
====
