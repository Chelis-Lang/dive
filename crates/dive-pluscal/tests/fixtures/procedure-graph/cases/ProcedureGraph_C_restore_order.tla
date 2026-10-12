---- MODULE ProcedureGraph_C_restore_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable c=2;
{
F:c:=a+1; call Second(a); R:x:=c+a; return;
};
procedure Second(b=1)
variable d=b+1;
{
S:x:=d; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
