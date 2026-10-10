---- MODULE ProcedureGraph_C_three_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); return;
};
procedure Second(b=1)

{
S:call Third(b); return;
};
procedure Third(c=0)

{
U:x:=c; V:return;
};
{
L: call First(2); E: skip;
}
} *)
====
