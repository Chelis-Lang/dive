---- MODULE Recursive_C_read_stack_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:x:=Len(stack); if(a>0){ call First(a-1); return; }else{ skip; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
