---- MODULE ProcedureGraph_C_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{
F:call Second(); R:return;
};
procedure Second()

{
S:x:=1; T:return;
};
{
L:call First(); E:skip;
}
} *)
====
