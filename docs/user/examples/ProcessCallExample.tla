---- MODULE ProcessCallExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Count(a = 0)
begin
  Entry: if a > 0 then call Count(a - 1); else x := x + 1; end if;
  Restore: x := x + 1; return;
end procedure;
process Worker = 1
begin
  Start: call Count(2);
  Finish: skip;
end process;
end algorithm; *)
====
