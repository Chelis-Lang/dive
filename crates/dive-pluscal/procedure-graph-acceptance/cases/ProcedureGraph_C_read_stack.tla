---- MODULE ProcedureGraph_C_read_stack ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:x:=Len(stack); return;
};
procedure Second(b=1)

{
S:x:=Len(stack); T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
