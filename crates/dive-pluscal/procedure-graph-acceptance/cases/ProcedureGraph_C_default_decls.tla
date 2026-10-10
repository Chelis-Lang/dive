---- MODULE ProcedureGraph_C_default_decls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a)
variable c;
{
F:call Second(a); R:return;
};
procedure Second(b=1)
variable d;
{
S: x:=b; T: return;
};
{
L: call First(2); E: skip;
}
} *)
====
