---- MODULE ProcedureCrc_C_repeat_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:call Put(2); M:call Put(x+1); E:skip;
}
} *)
====
