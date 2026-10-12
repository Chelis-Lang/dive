---- MODULE Recursive_C_false_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(FALSE){ call First(a); }else{ x:=a; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
