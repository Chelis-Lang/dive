---- MODULE Procedure_C_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable b=1;
{
P: x:=a+b; R: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
