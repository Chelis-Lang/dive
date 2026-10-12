---- MODULE ProcessProcedure_C_member_process_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker \in {1,2})
variable y=self;
{ L:call First(y); E:y:=x; }
} *)
====
