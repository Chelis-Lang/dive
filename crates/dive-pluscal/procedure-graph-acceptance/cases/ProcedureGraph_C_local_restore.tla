---- MODULE ProcedureGraph_C_local_restore ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable c=9;
{
F:c:=a+1; call Second(c); R:x:=c; return;
};
procedure Second(b=1)
variable d=4;
{
S:d:=b+1; x:=d; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
