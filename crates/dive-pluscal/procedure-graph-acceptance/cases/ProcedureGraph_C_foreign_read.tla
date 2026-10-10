---- MODULE ProcedureGraph_C_foreign_read ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:x:=d; return;
};
procedure Second(b=1)
variable d=4;
{
S:x:=b; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
