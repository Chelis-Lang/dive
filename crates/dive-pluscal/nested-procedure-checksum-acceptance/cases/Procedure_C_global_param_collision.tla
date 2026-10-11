---- MODULE Procedure_C_global_param_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(x=0)

{
P: x:=1; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
