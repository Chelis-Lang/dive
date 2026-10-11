---- MODULE ProcedureCrc_C_either_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:either {call Put(2); goto E;} or {call Put(3); goto E; }; E:skip;
}
} *)
====
