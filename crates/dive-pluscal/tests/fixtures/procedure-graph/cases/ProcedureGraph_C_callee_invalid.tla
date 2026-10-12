---- MODULE ProcedureGraph_C_callee_invalid ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)

{
S:x:=Missing; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
