---- MODULE ProcedureGraph_C_three_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)

{
S:call Third(b+1); T:x:=x+b; return;
};
procedure Third(c=0)
variable d=c+1;
{
U:x:=d; V:return;
};
{
L: call First(2); E: skip;
}
} *)
====
