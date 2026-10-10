---- MODULE Recursive_P_self_tail_assign_global ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; if a>0 then call First(a-1); return; else skip; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
