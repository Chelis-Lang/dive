---- MODULE Recursive_C_self_while ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:while(a>0){ call First(a-1); C:a:=a-1; }; R:x:=x+1; return;
};
{
L:call First(2); E:skip;
}
} *)
====
