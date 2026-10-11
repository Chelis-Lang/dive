---- MODULE Procedure_C_param_local_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable a=1;
{
P: x:=a; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
