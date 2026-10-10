---- MODULE Recursive_P_self_while ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:while a>0 do call First(a-1); C:a:=a-1; end while; R:x:=x+1; return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
