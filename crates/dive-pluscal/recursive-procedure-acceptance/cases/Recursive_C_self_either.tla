---- MODULE Recursive_C_self_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ either{call First(a-1);}or{x:=a; }; }else{ x:=0; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
