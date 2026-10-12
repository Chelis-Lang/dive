---- MODULE Recursive_C_self_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:with(n=a-1){ if(a>0){ call First(n); }else{ x:=x+1; }; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
