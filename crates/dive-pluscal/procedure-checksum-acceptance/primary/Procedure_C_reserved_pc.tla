---- MODULE Procedure_C_reserved_pc ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(pc=0)

{
P: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
