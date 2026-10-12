---- MODULE ProcessHeader_P_member_while_false ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker \in {1,2}
begin
L:while FALSE do call First(2); end while; E:skip;
end process;

end algorithm; *)
====
