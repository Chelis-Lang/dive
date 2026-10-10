---- MODULE ProcedureGraph_C_implicit_callee ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)

{
S:x:=b;
};
{
L: call First(2); E: skip;
}
} *)
====
