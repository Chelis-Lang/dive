---- MODULE Recursive_C_self_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); goto R; }else{ x:=0; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
