---- MODULE Recursive_C_read_stack_depth ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:x:=Len(stack); if(a>0){ call First(a-1); }else{ skip; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
