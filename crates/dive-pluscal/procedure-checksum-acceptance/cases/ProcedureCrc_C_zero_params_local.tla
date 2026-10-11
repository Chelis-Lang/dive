---- MODULE ProcedureCrc_C_zero_params_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put()
variable t=1;
{
P:x:=t; R:return;
};
{
L:call Put(); E:skip;
}
} *)
====
