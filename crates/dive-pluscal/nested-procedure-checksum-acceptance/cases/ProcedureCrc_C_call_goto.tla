---- MODULE ProcedureCrc_C_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:call Put(2); goto E; E:skip;
}
} *)
====
