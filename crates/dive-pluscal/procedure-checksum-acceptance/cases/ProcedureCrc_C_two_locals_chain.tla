---- MODULE ProcedureCrc_C_two_locals_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variables t=a+1,u=t+1;
{
P:x:=u; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
====
