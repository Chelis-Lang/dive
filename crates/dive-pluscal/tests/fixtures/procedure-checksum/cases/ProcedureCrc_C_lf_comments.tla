---- MODULE ProcedureCrc_C_lf_comments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable t=(* initial *) a+1;
{
P:x:=(* actual *) t; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
====
