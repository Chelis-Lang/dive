---- MODULE ProcedureGraph_C_parallel_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{
F:a:=c || c:=a; R:call Second(a,c); Z:return;
};
procedure Second(b=1,d=2)

{
S:b:=d || d:=b; T:x:=b+d; return;
};
{
L:call First(2,3); E:skip;
}
} *)
====
