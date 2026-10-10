---- MODULE ProcedureGraph_C_separate_local_init ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)
variable d=b+1,e=d+1;
{
S:x:=d+e; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
