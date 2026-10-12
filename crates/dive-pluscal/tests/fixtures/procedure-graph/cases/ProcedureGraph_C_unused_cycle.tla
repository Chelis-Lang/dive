---- MODULE ProcedureGraph_C_unused_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure Second(b=1)

{
S:call Second(b); T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
