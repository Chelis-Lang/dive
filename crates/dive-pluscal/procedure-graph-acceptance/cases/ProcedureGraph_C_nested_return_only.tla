---- MODULE ProcedureGraph_C_nested_return_only ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)

{
S:return;
};
{
L: call First(2); E: skip;
}
} *)
====
