---- MODULE ProcedureGraph_C_four_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)

{
S:call Third(b); T:return;
};
procedure Third(c=0)

{
U:call Fourth(c); V:return;
};
procedure Fourth(d=0)

{
W:x:=d; Q:return;
};
{
L: call First(2); E: skip;
}
} *)
====
