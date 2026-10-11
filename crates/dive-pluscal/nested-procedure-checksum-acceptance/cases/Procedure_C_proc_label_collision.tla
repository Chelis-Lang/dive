---- MODULE Procedure_C_proc_label_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
Put: x:=a; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
