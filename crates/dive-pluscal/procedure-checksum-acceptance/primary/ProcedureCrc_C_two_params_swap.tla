---- MODULE ProcedureCrc_C_two_params_swap ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0,b=1)

{
P:x:=a+b; R:return;
};
{
L:call Put(b,a); E:skip;
}
} *)
====
