---- MODULE ProcedureCrc_C_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable t;
{
P:t:=a; Q:x:=t; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
====
