---- MODULE Procedure_C_operator_names ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(Next=0)

{
P: x:=Next; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
