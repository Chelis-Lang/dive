---- MODULE ProcessProcedure_C_member_process_local_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:y:=a; R:return; };

process (Worker \in {1,2})
variable y=self;
{ L:call First(2); E:skip; }
} *)
====
