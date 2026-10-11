---- MODULE Procedure_C_local_update ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable b=1;
{
P: b:=a+1; x:=b; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
