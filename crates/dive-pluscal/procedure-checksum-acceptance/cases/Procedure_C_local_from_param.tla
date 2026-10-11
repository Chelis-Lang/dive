---- MODULE Procedure_C_local_from_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable b=a+1;
{
P: x:=b; R: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
