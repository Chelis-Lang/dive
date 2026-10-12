---- MODULE ProcedureCrc_C_local_from_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable t=a+1;
{
P:x:=t; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
====
