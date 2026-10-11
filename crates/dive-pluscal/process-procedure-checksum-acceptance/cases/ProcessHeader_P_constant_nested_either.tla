---- MODULE ProcessHeader_P_constant_nested_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:either call First(2); or call First(3); end either; E:skip;
end process;

end algorithm; *)
====
