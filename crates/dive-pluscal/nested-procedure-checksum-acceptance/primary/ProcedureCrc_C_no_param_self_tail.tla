---- MODULE ProcedureCrc_C_no_param_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put()

{
P:call Put(); return;
};
{
L:call Put(); E:skip;
}
} *)
====
