---- MODULE ProcedureCrc_C_zero_slots ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put()

{
P:x:=2; R:return;
};
{
L:call Put(); E:skip;
}
} *)
====
